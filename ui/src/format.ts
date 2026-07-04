// Value formatters. Millicores and bytes are the wire units.

export function fmtCores(millicores: number): string {
  if (!isFinite(millicores)) return '–'
  if (Math.abs(millicores) >= 1000) return `${(millicores / 1000).toFixed(2)} cores`
  return `${millicores.toFixed(0)} mc`
}

export function fmtBytes(bytes: number): string {
  if (!isFinite(bytes)) return '–'
  const abs = Math.abs(bytes)
  if (abs >= 2 ** 40) return `${(bytes / 2 ** 40).toFixed(2)} TiB`
  if (abs >= 2 ** 30) return `${(bytes / 2 ** 30).toFixed(2)} GiB`
  if (abs >= 2 ** 20) return `${(bytes / 2 ** 20).toFixed(1)} MiB`
  if (abs >= 1024) return `${(bytes / 1024).toFixed(0)} KiB`
  return `${bytes.toFixed(0)} B`
}

export function fmtPercent(ratio: number): string {
  if (!isFinite(ratio)) return '–'
  return `${(ratio * 100).toFixed(1)}%`
}

export function fmtHours(h: number): string {
  if (!isFinite(h)) return '–'
  return h >= 100 ? h.toFixed(0) : h.toFixed(2)
}

export function fmtCount(n: number): string {
  return n.toLocaleString()
}
