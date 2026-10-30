export function formatSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit++
  }
  const digits = value >= 100 || unit === 0 ? 0 : 1
  return `${value.toFixed(digits)} ${units[unit]}`
}

export function shortPath(path: string, max = 64): string {
  if (path.length <= max) return path
  const head = Math.floor((max - 3) / 2)
  return `${path.slice(0, head)}...${path.slice(-head)}`
}
