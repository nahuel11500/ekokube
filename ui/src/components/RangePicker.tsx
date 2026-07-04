export interface RangePreset {
  label: string
  seconds: number
}

export const PRESETS: RangePreset[] = [
  { label: '1h', seconds: 3600 },
  { label: '6h', seconds: 6 * 3600 },
  { label: '24h', seconds: 24 * 3600 },
  { label: '7d', seconds: 7 * 24 * 3600 },
  { label: '30d', seconds: 30 * 24 * 3600 },
]

interface Props {
  selected: RangePreset
  onSelect: (p: RangePreset) => void
}

export function RangePicker({ selected, onSelect }: Props) {
  return (
    <div className="range-picker" role="group" aria-label="Time range">
      {PRESETS.map((p) => (
        <button
          key={p.label}
          className={p.seconds === selected.seconds ? 'active' : ''}
          onClick={() => onSelect(p)}
        >
          {p.label}
        </button>
      ))}
    </div>
  )
}
