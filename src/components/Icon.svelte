<script lang="ts">
  // Small, consistent 16px line-icon set. No external icon dependency: paths are
  // kept here so the portable bundle stays self-contained.
  type IconName =
    | 'search'
    | 'discover'
    | 'scan'
    | 'layers'
    | 'tag'
    | 'sparkles'
    | 'settings'
    | 'play'
    | 'folder'
    | 'pencil'
    | 'plus'
    | 'close'
    | 'minimize'
    | 'maximize'
    | 'check'
    | 'chevron-down'

  let { name, size = 16 }: { name: IconName; size?: number } = $props()

  const paths: Record<IconName, string[]> = {
    search: ['M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14Z', 'm16.5 16.5 4 4'],
    discover: [
      'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Z',
      'm15.5 8.5-2.2 5-5 2.2 2.2-5 5-2.2Z'
    ],
    scan: [
      'M4 9V5a1 1 0 0 1 1-1h4',
      'M20 9V5a1 1 0 0 0-1-1h-4',
      'M4 15v4a1 1 0 0 0 1 1h4',
      'M20 15v4a1 1 0 0 1-1 1h-4',
      'M4 12h16'
    ],
    layers: ['m12 3 9 5-9 5-9-5 9-5Z', 'm3 12 9 5 9-5', 'm3 16 9 5 9-5'],
    tag: ['M3 11V4a1 1 0 0 1 1-1h7l10 10-8 8L3 11Z', 'M7.5 7.5h.01'],
    sparkles: [
      'm12 3 1.6 4.4L18 9l-4.4 1.6L12 15l-1.6-4.4L6 9l4.4-1.6L12 3Z',
      'm18.5 14 .8 2.2 2.2.8-2.2.8-.8 2.2-.8-2.2-2.2-.8 2.2-.8.8-2.2Z'
    ],
    settings: [
      'M4 7h10',
      'M18 7h2',
      'M4 17h2',
      'M10 17h10',
      'M16 5v4',
      'M8 15v4'
    ],
    play: ['m8 5 11 7-11 7V5Z'],
    folder: ['M3 7a1 1 0 0 1 1-1h5l2 2h8a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V7Z'],
    pencil: ['M4 20h4L20 8l-4-4L4 16v4Z', 'm14 6 4 4'],
    plus: ['M12 5v14', 'M5 12h14'],
    close: ['M6 6l12 12', 'M18 6 6 18'],
    minimize: ['M5 12h14'],
    maximize: ['M5 5h14v14H5V5Z'],
    check: ['m5 12 5 5L20 7'],
    'chevron-down': ['m6 9 6 6 6-6']
  }
</script>

<svg
  class="ph-icon-svg"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.7"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  {#each paths[name] as d (d)}
    <path {d} />
  {/each}
</svg>
