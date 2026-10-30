<script lang="ts">
  import { untrack } from 'svelte'
  import type { AppView, Collection } from '../lib/types'
  import type { Translator } from '../lib/i18n'
  import Icon from './Icon.svelte'

  let {
    collections,
    selectedApps,
    t,
    onsave,
    onclose
  }: {
    collections: Collection[]
    selectedApps: AppView[]
    t: Translator
    onsave: (collections: Collection[]) => void
    onclose: () => void
  } = $props()

  let list = $state<Collection[]>(untrack(() => JSON.parse(JSON.stringify(collections)) as Collection[]))
  let draft = $state('')

  function newId(): string {
    return typeof crypto !== 'undefined' && 'randomUUID' in crypto
      ? crypto.randomUUID()
      : `c-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
  }

  function create() {
    const name = draft.trim()
    if (!name) return
    list = [
      ...list,
      { id: newId(), name, appIds: selectedApps.map((app) => app.id) }
    ]
    draft = ''
  }

  function rename(item: Collection, name: string) {
    list = list.map((entry) => (entry.id === item.id ? { ...entry, name } : entry))
  }

  function remove(item: Collection) {
    list = list.filter((entry) => entry.id !== item.id)
  }

  function includesSelected(item: Collection): boolean {
    return selectedApps.length > 0 && selectedApps.every((app) => item.appIds.includes(app.id))
  }

  function toggleSelected(item: Collection) {
    const ids = selectedApps.map((app) => app.id)
    list = list.map((entry) =>
      entry.id !== item.id
        ? entry
        : {
            ...entry,
            appIds: includesSelected(entry)
              ? entry.appIds.filter((id) => !ids.includes(id))
              : [...new Set([...entry.appIds, ...ids])]
          }
    )
  }

  function save() {
    onsave(
      list
        .map((item) => ({ ...item, name: item.name.trim() }))
        .filter((item) => item.name.length > 0)
    )
  }
</script>

<div
  class="ph-overlay"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget) onclose()
  }}
>
  <div
    class="ph-settings ph-collection-modal"
    role="dialog"
    aria-modal="true"
    aria-label={t('collections.title')}
    data-testid="collection-modal"
  >
    <div class="ph-settings-head">
      <strong>{t('collections.title')}</strong>
      <button class="ph-btn" data-testid="collection-close" onclick={onclose}>
        {t('settings.close')}
      </button>
    </div>
    <div class="ph-collection-body">
      {#if !list.length && !selectedApps.length}
        <p class="ph-hint">{t('collections.empty')}</p>
      {/if}
      {#each list as item (item.id)}
        <div class="ph-collection-row" data-testid="collection-row">
          <input
            class="ph-input"
            value={item.name}
            aria-label={t('collections.name')}
            oninput={(event) => rename(item, event.currentTarget.value)}
          />
          <span class="ph-hint">{t('collections.count', { n: item.appIds.length })}</span>
          {#if selectedApps.length}
            <label class="ph-collection-check">
              <input
                class="ph-check"
                type="checkbox"
                data-testid="collection-include"
                checked={includesSelected(item)}
                onchange={() => toggleSelected(item)}
              />
              {t('collections.includeSelected')}
            </label>
          {/if}
          <button
            class="ph-btn ph-danger"
            type="button"
            data-testid="collection-delete"
            title={t('collections.delete')}
            aria-label={t('collections.delete')}
            onclick={() => remove(item)}
          >×</button>
        </div>
      {/each}
      <div class="ph-collection-new">
        <input
          class="ph-input"
          data-testid="collection-new-name"
          placeholder={t('collections.name')}
          bind:value={draft}
          onkeydown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault()
              create()
            }
          }}
        />
        <button class="ph-btn" type="button" data-testid="collection-create" onclick={create}>
          <Icon name="plus" />{t('collections.create')}
        </button>
      </div>
      {#if selectedApps.length}
        <p class="ph-hint" data-testid="collection-selected-hint">
          {t('collections.selectedApps', { n: selectedApps.length })}
        </p>
      {/if}
    </div>
    <div class="ph-collection-foot">
      <button class="ph-btn" type="button" onclick={onclose}>{t('settings.close')}</button>
      <button class="ph-btn accent" type="button" data-testid="collection-save" onclick={save}>
        {t('collections.save')}
      </button>
    </div>
  </div>
</div>

<style>
  .ph-collection-modal {
    width: min(560px, 94vw);
    height: auto;
    max-height: min(76vh, 620px);
  }

  .ph-collection-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow-y: auto;
    padding: 14px 18px;
  }

  .ph-collection-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .ph-collection-row .ph-input {
    min-width: 0;
    flex: 1;
  }

  .ph-collection-check {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--ph-text-muted);
    white-space: nowrap;
  }

  .ph-collection-new {
    display: flex;
    gap: 8px;
    padding-top: 4px;
  }

  .ph-collection-new .ph-input {
    min-width: 0;
  }

  .ph-collection-foot {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 18px;
    border-top: var(--ph-border-width) solid var(--ph-border);
  }
</style>
