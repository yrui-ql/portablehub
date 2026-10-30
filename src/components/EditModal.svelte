<script lang="ts">
  // REQ-0013 · 应用编辑对话框。
  //
  // 契约（由 App.svelte 固定，不要改）：
  //   app    当前要编辑的应用（对话框每次打开都挂载新实例）
  //   t      文案函数
  //   onclose 关闭对话框；保存成功后由父层卸载本组件，因此成功时不调用 onclose
  //   onsave  提交补丁；父层失败时会 throw，此处捕获并保持对话框打开
  import { onMount, untrack } from 'svelte'
  import type { AppEditPatch, AppView } from '../lib/types'
  import type { Translator } from '../lib/i18n'
  import { pickDirectory } from '../lib/desktop'
  import { baseName, dirName, isAbsolutePath, isDriveOnly, joinPath } from '../lib/path'
  import { CATEGORY_ORDER } from '../lib/categories'
  import InfoTip from './InfoTip.svelte'

  let {
    app,
    t,
    onclose,
    onsave
  }: {
    app: AppView
    t: Translator
    onclose: () => void
    onsave: (patch: AppEditPatch) => Promise<void>
  } = $props()

  // 打开时快照一次当前值（untrack：之后父层刷新不会覆盖用户输入；保存成功后父层会卸载本组件）。
  let name = $state(untrack(() => app.name))
  let path = $state(untrack(() => app.path))
  let argsText = $state(untrack(() => app.args.join(' ')))
  let workDir = $state(untrack(() => app.workDir))
  // BUG-0023: apps that need elevation are flagged here; the backend launches
  // them through the Windows "runas" path. Older fixtures may omit the field.
  let runAsAdmin = $state(untrack(() => app.runAsAdmin === true))
  let category = $state(untrack(() => app.category ?? ''))
  let error = $state('')
  let saving = $state(false)
  let nameEl: HTMLInputElement | undefined = $state()

  onMount(() => {
    const previous = document.activeElement as HTMLElement | null
    nameEl?.focus()
    // Esc 关闭；capture 阶段拦下，避免父层的全局 Esc 处理（清空搜索/选中）同时生效。
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      if (!saving) onclose()
    }
    window.addEventListener('keydown', onKey, true)
    return () => {
      window.removeEventListener('keydown', onKey, true)
      // Restore focus to the card/menu that opened the dialog.
      previous?.focus?.()
    }
  })

  function parseArgs(text: string): string[] {
    const trimmed = text.trim()
    return trimmed ? trimmed.split(/\s+/) : []
  }

  function errorText(reason: unknown): string {
    return reason instanceof Error ? reason.message : String(reason)
  }

  // pickDirectory 只能选文件夹：路径字段保留当前可执行文件名，只替换所在目录。
  // 没有文件名（空路径 / 盘根 / 目录形式）时直接用所选目录。
  async function browsePath() {
    const current = path.trim()
    const file = baseName(current)
    const dir = dirName(current)
    const picked = await pickDirectory(isAbsolutePath(dir) ? dir : undefined)
    if (!picked) return
    path = !file || isDriveOnly(file) ? picked : joinPath(picked, file)
  }

  async function browseWorkDir() {
    const current = workDir.trim()
    const picked = await pickDirectory(isAbsolutePath(current) ? current : undefined)
    if (picked) workDir = picked
  }

  async function submit() {
    if (saving) return
    if (!name.trim()) {
      error = t('edit.nameRequired')
      return
    }
    if (!path.trim()) {
      error = t('edit.pathRequired')
      return
    }
    error = ''
    saving = true
    try {
      await onsave({
        name: name.trim(),
        path: path.trim(),
        args: parseArgs(argsText),
        workDir: workDir.trim(),
        runAsAdmin,
        category
      })
    } catch (reason) {
      // 父层失败：保持对话框打开，显示可读错误。
      error = t('edit.saveFailed', { error: errorText(reason) })
    } finally {
      saving = false
    }
  }

  function onSubmit(event: SubmitEvent) {
    event.preventDefault()
    void submit()
  }
</script>

<div
  class="ph-overlay"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget && !saving) onclose()
  }}
>
  <div
    class="ph-settings ph-edit"
    role="dialog"
    aria-modal="true"
    aria-label={t('edit.title')}
    data-testid="edit-modal"
  >
    <div class="ph-settings-head">
      <strong>{t('edit.title')}</strong>
    </div>

    <form class="ph-edit-body" onsubmit={onSubmit}>
      <section class="ph-section">
        <div class="ph-row">
          <label for="edit-name">{t('edit.name')}</label>
          <input
            id="edit-name"
            class="ph-input"
            data-testid="edit-name"
            bind:this={nameEl}
            bind:value={name}
            spellcheck="false"
            autocomplete="off"
            disabled={saving}
          />
        </div>

        <div class="ph-row">
          <label for="edit-path">{t('edit.path')}</label>
          <input
            id="edit-path"
            class="ph-input"
            data-testid="edit-path"
            bind:value={path}
            spellcheck="false"
            autocomplete="off"
            disabled={saving}
          />
          <button type="button" class="ph-btn" data-testid="edit-path-browse" disabled={saving} onclick={browsePath}>
            {t('edit.browse')}
          </button>
          <InfoTip testid="edit-path-info" label={t('edit.pathHintLabel')} text={t('edit.pathHint')} />
        </div>

        <div class="ph-row">
          <label for="edit-args">{t('edit.args')}</label>
          <input
            id="edit-args"
            class="ph-input"
            data-testid="edit-args"
            bind:value={argsText}
            spellcheck="false"
            autocomplete="off"
            disabled={saving}
          />
          <InfoTip testid="edit-args-info" label={t('edit.argsHintLabel')} text={t('edit.argsHint')} />
        </div>

        <div class="ph-row">
          <label for="edit-workdir">{t('edit.workDir')}</label>
          <input
            id="edit-workdir"
            class="ph-input"
            data-testid="edit-workdir"
            bind:value={workDir}
            spellcheck="false"
            autocomplete="off"
            disabled={saving}
          />
          <button type="button" class="ph-btn" data-testid="edit-workdir-browse" disabled={saving} onclick={browseWorkDir}>
            {t('edit.browse')}
          </button>
          <InfoTip testid="edit-workdir-info" label={t('edit.workDirHintLabel')} text={t('edit.workDirHint')} />
        </div>

        <div class="ph-row">
          <label for="edit-category">{t('edit.category')}</label>
          <select
            id="edit-category"
            class="ph-select"
            data-testid="edit-category"
            bind:value={category}
            disabled={saving}
          >
            <option value="">{t('category.unclassified')}</option>
            {#each CATEGORY_ORDER as value (value)}
              <option value={value}>{t(`category.${value}`)}</option>
            {/each}
          </select>
        </div>

        <div class="ph-row">
          <label for="edit-run-as-admin">{t('edit.runAsAdmin')}</label>
          <input
            id="edit-run-as-admin"
            type="checkbox"
            class="ph-check"
            data-testid="edit-run-as-admin"
            bind:checked={runAsAdmin}
            disabled={saving}
          />
          <InfoTip
            testid="edit-run-as-admin-info"
            label={t('edit.runAsAdminHintLabel')}
            text={t('edit.runAsAdminHint')}
          />
        </div>
      </section>

      <div class="ph-edit-foot">
        {#if error}
          <p class="ph-hint ph-danger" role="alert" data-testid="edit-error">{error}</p>
        {/if}
        <div class="ph-edit-actions">
          <button type="button" class="ph-btn" data-testid="edit-cancel" disabled={saving} onclick={onclose}>
            {t('edit.cancel')}
          </button>
          <button type="submit" class="ph-btn accent" data-testid="edit-save" disabled={saving}>
            {saving ? t('edit.saving') : t('edit.save')}
          </button>
        </div>
      </div>
    </form>
  </div>
</div>

<style>
  .ph-edit {
    width: min(620px, 94vw);
  }

  .ph-edit-body {
    margin: 0;
  }

  .ph-edit-body .ph-section {
    border-bottom: 0;
    padding-bottom: 4px;
  }

  .ph-edit-foot {
    padding: 0 14px 14px;
    border-top: var(--ph-border-width) solid var(--ph-border);
    padding-top: 12px;
  }

  .ph-edit-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    flex-wrap: wrap;
  }
</style>
