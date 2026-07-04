// Row-height usage sparkline: one SVG polyline per row (no chart-library
// instances — hundreds of rows must stay cheap).

interface Props {
  values: number[]
  width?: number
  height?: number
}

export function Sparkline({ values, width = 96, height = 22 }: Props) {
  if (values.length < 2) return <span className="dim">–</span>
  const max = Math.max(...values, 1e-9)
  const stepX = width / (values.length - 1)
  const points = values
    .map((v, i) => `${(i * stepX).toFixed(1)},${(height - 2 - (v / max) * (height - 4)).toFixed(1)}`)
    .join(' ')
  return (
    <svg
      className="sparkline"
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      aria-hidden="true"
    >
      <polyline points={points} fill="none" strokeWidth="1.5" />
    </svg>
  )
}
