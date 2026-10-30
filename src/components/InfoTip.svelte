<script lang="ts">
  // Progressive disclosure: supplementary help lives behind a small info
  // affordance instead of a wall of inline text.
  //
  // The bubble is portalled to <body> and positioned with `position: fixed`.
  // A tooltip rendered inside the settings scroll container was clipped by
  // `overflow` / painted under sibling panels — the exact "tooltip hidden by
  // another window" bug.
  let {
    label,
    text,
    testid = 'info-tip',
    align = 'right'
  }: { label: string; text: string; testid?: string; align?: 'left' | 'right' } = $props()

  let open = $state(false)
  let btnEl = $state<HTMLButtonElement | null>(null)
  let tipEl: HTMLDivElement | null = null
  const tipId = `ph-info-tip-${Math.random().toString(36).slice(2)}`

  function place() {
    if (!btnEl || !tipEl) return
    const rect = btnEl.getBoundingClientRect()
    const margin = 8
    const width = Math.min(300, window.innerWidth - margin * 2)
    tipEl.style.width = `${width}px`
    const height = tipEl.offsetHeight
    let top = rect.top - height - 8
    if (top < margin) top = rect.bottom + 8
    let left = align === 'left' ? rect.left : rect.right - width
    left = Math.max(margin, Math.min(left, window.innerWidth - width - margin))
    tipEl.style.top = `${top}px`
    tipEl.style.left = `${left}px`
  }

  function removeTip() {
    tipEl?.remove()
    tipEl = null
    btnEl?.removeAttribute('aria-describedby')
  }

  function closeOnPointer(event: PointerEvent) {
    const target = event.target as Node | null
    if (target === btnEl || tipEl?.contains(target)) return
    open = false
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      open = false
      ;(event.currentTarget as HTMLElement).blur()
    }
  }

  $effect(() => {
    const visible = open
    const content = text
    if (!visible) {
      removeTip()
      return
    }
    if (!tipEl) {
      tipEl = document.createElement('div')
      tipEl.className = 'ph-info-card ph-info-portal'
      tipEl.id = tipId
      tipEl.setAttribute('role', 'tooltip')
      document.body.appendChild(tipEl)
    }
    tipEl.textContent = content
    btnEl?.setAttribute('aria-describedby', tipId)
    place()

    const reposition = () => place()
    window.addEventListener('scroll', reposition, true)
    window.addEventListener('resize', reposition)
    document.addEventListener('pointerdown', closeOnPointer, true)
    return () => {
      window.removeEventListener('scroll', reposition, true)
      window.removeEventListener('resize', reposition)
      document.removeEventListener('pointerdown', closeOnPointer, true)
    }
  })

  $effect(() => () => removeTip())
</script>

<span class="ph-infotip">
  <button
    type="button"
    class="ph-info-btn"
    bind:this={btnEl}
    data-testid={testid}
    aria-label={label}
    aria-expanded={open}
    onmouseenter={() => (open = true)}
    onmouseleave={() => (open = false)}
    onfocus={() => (open = true)}
    onblur={() => (open = false)}
    onkeydown={onKeydown}
    onclick={() => (open = !open)}
  >
    i
  </button>
</span>

<style>
  .ph-infotip {
    position: relative;
    display: inline-flex;
    flex: 0 0 auto;
  }

  .ph-info-btn {
    width: 18px;
    height: 18px;
    padding: 0;
    border: 1px solid var(--ph-border-light);
    border-radius: 0;
    background: var(--ph-panel);
    color: var(--ph-text-muted);
    font-size: 11px;
    font-style: italic;
    line-height: 16px;
    cursor: help;
  }

  .ph-info-btn:hover,
  .ph-info-btn:focus-visible {
    border-color: var(--ph-accent);
    color: var(--ph-accent);
    outline: none;
  }

  :global(.ph-info-portal) {
    position: fixed;
    z-index: 2147483000;
    box-sizing: border-box;
    padding: 8px 10px;
    border: var(--ph-border-width) solid var(--ph-border-light);
    background: var(--ph-panel);
    box-shadow: 4px 4px 0 var(--ph-shadow);
    color: var(--ph-text);
    font-size: 11px;
    line-height: 1.55;
    text-align: left;
    white-space: normal;
    pointer-events: none;
  }
</style>
