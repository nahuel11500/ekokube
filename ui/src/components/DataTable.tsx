// Virtualized table: only visible rows are rendered, sorting is delegated to
// the server via onSort (rows arrive pre-sorted and paginated).

import { useRef } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'

export interface Column {
  key: string
  label: string
  width: string
  numeric?: boolean
  sortKey?: string
  render?: (row: Record<string, unknown>) => React.ReactNode
}

interface Props {
  columns: Column[]
  rows: Record<string, unknown>[]
  sortBy: string
  order: 'asc' | 'desc'
  onSort: (sortKey: string) => void
  onRowClick?: (row: Record<string, unknown>) => void
  maxHeight?: number
}

const ROW_HEIGHT = 33

export function DataTable({
  columns,
  rows,
  sortBy,
  order,
  onSort,
  onRowClick,
  maxHeight = 560,
}: Props) {
  const bodyRef = useRef<HTMLDivElement>(null)
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => bodyRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  })

  const grid = { gridTemplateColumns: columns.map((c) => c.width).join(' ') }

  return (
    <div className="dtable">
      <div className="dtable-header" style={grid}>
        {columns.map((c) => (
          <div
            key={c.key}
            className={`cell ${c.numeric ? 'num' : ''} ${sortBy === c.sortKey ? 'sorted' : ''}`}
            onClick={() => c.sortKey && onSort(c.sortKey)}
          >
            {c.label}
            {sortBy === c.sortKey ? (order === 'desc' ? ' ↓' : ' ↑') : ''}
          </div>
        ))}
      </div>
      <div
        className="dtable-body"
        ref={bodyRef}
        style={{ maxHeight, height: Math.min(rows.length * ROW_HEIGHT, maxHeight) }}
      >
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {virtualizer.getVirtualItems().map((item) => {
            const row = rows[item.index]
            return (
              <div
                key={item.key}
                className={`dtable-row ${onRowClick ? 'link' : ''}`}
                style={{ ...grid, transform: `translateY(${item.start}px)`, height: ROW_HEIGHT }}
                onClick={onRowClick ? () => onRowClick(row) : undefined}
              >
                {columns.map((c) => (
                  <div key={c.key} className={`cell ${c.numeric ? 'num' : ''}`}>
                    {c.render ? c.render(row) : String(row[c.key] ?? '')}
                  </div>
                ))}
              </div>
            )
          })}
        </div>
      </div>
      {rows.length === 0 && <div className="empty">No data in this time range</div>}
    </div>
  )
}
