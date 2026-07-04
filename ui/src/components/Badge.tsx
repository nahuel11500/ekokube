// Status badge: icon + label, never color alone (status colors are reserved
// for state and always paired with text).

interface Props {
  level: 'good' | 'warning' | 'serious' | 'critical'
  children: React.ReactNode
  title?: string
}

const ICONS = { good: '✓', warning: '△', serious: '▲', critical: '✕' }

export function Badge({ level, children, title }: Props) {
  return (
    <span className={`badge badge-${level}`} title={title}>
      <span aria-hidden="true">{ICONS[level]}</span> {children}
    </span>
  )
}
