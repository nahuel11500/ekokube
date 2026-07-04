import { useState } from 'react'
import { api, toRows, type TimeRange } from '../api'
import { useApi } from '../hooks'
import { fmtBytes, fmtCores } from '../format'
import { DataTable, type Column } from '../components/DataTable'

function saturation(used: number, capacity: number) {
  const ratio = capacity > 0 ? Math.min(used / capacity, 1) : 0
  return (
    <span className="mini-bar" title={`${(ratio * 100).toFixed(0)}% of capacity`}>
      <i style={{ width: `${ratio * 100}%` }} />
    </span>
  )
}

const COLUMNS: Column[] = [
  { key: 'node', label: 'Node', width: 'minmax(170px, 1.5fr)' },
  { key: 'pods', label: 'Pods', width: '60px', numeric: true },
  {
    key: 'cpu_used_avg_millicores',
    label: 'CPU used avg',
    width: 'minmax(150px, 1.2fr)',
    numeric: true,
    render: (r) => (
      <>
        {fmtCores(r.cpu_used_avg_millicores as number)}
        {saturation(r.cpu_used_avg_millicores as number, r.cpu_capacity_millicores as number)}
      </>
    ),
  },
  {
    key: 'cpu_used_max_millicores',
    label: 'CPU max',
    width: 'minmax(90px, 1fr)',
    numeric: true,
    render: (r) => fmtCores(r.cpu_used_max_millicores as number),
  },
  {
    key: 'cpu_requested_avg_millicores',
    label: 'CPU requested',
    width: 'minmax(110px, 1fr)',
    numeric: true,
    render: (r) => fmtCores(r.cpu_requested_avg_millicores as number),
  },
  {
    key: 'cpu_allocatable_millicores',
    label: 'CPU alloc',
    width: 'minmax(90px, 1fr)',
    numeric: true,
    render: (r) => fmtCores(r.cpu_allocatable_millicores as number),
  },
  {
    key: 'mem_used_avg_bytes',
    label: 'Mem used avg',
    width: 'minmax(150px, 1.2fr)',
    numeric: true,
    render: (r) => (
      <>
        {fmtBytes(r.mem_used_avg_bytes as number)}
        {saturation(r.mem_used_avg_bytes as number, r.mem_total_bytes as number)}
      </>
    ),
  },
  {
    key: 'mem_requested_avg_bytes',
    label: 'Mem requested',
    width: 'minmax(110px, 1fr)',
    numeric: true,
    render: (r) => fmtBytes(r.mem_requested_avg_bytes as number),
  },
  {
    key: 'mem_allocatable_bytes',
    label: 'Mem alloc',
    width: 'minmax(90px, 1fr)',
    numeric: true,
    render: (r) => fmtBytes(r.mem_allocatable_bytes as number),
  },
]

const ROW_KEYS = COLUMNS.map((c) => c.key).concat([
  'cpu_capacity_millicores',
  'mem_total_bytes',
])

export function Nodes({ range }: { range: TimeRange }) {
  // Client-side sort: the node list is small (one row per node).
  const [sortBy, setSortBy] = useState('node')
  const [order, setOrder] = useState<'asc' | 'desc'>('asc')
  const { data, error, loading } = useApi(() => api.nodes(range), [range.from, range.to])

  if (error) return <div className="error">{error}</div>
  let rows = data ? toRows(data, ROW_KEYS) : []
  rows = [...rows].sort((a, b) => {
    const av = a[sortBy] as number | string
    const bv = b[sortBy] as number | string
    const cmp = typeof av === 'number' ? av - (bv as number) : String(av).localeCompare(String(bv))
    return order === 'asc' ? cmp : -cmp
  })

  const onSort = (key: string) => {
    if (key === sortBy) setOrder(order === 'desc' ? 'asc' : 'desc')
    else {
      setSortBy(key)
      setOrder('desc')
    }
  }

  const columns = COLUMNS.map((c) => ({ ...c, sortKey: c.key }))

  return (
    <div className="card">
      <h3>Nodes — saturation &amp; bin-packing headroom</h3>
      {loading && !data ? (
        <div className="loading">Loading…</div>
      ) : (
        <DataTable columns={columns} rows={rows} sortBy={sortBy} order={order} onSort={onSort} />
      )}
    </div>
  )
}
