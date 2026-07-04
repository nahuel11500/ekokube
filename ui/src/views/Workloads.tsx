import { useState } from 'react'
import { api, toRows, type TimeRange } from '../api'
import { useApi } from '../hooks'
import { fmtBytes, fmtCores, fmtPercent } from '../format'
import { DataTable, type Column } from '../components/DataTable'

const COLUMNS: Column[] = [
  {
    key: 'workload_name',
    label: 'Workload',
    width: 'minmax(180px, 1.6fr)',
    sortKey: undefined,
    render: (r) => (
      <>
        {String(r.workload_name)} <span className="dim">{String(r.workload_kind)}</span>
      </>
    ),
  },
  { key: 'namespace', label: 'Namespace', width: 'minmax(120px, 1fr)' },
  { key: 'pods', label: 'Pods', width: '60px', numeric: true, sortKey: 'pods' },
  {
    key: 'cpu_usage_avg_millicores',
    label: 'CPU avg',
    width: 'minmax(95px, 1fr)',
    numeric: true,
    sortKey: 'cpu_usage_avg',
    render: (r) => fmtCores(r.cpu_usage_avg_millicores as number),
  },
  {
    key: 'cpu_p95_per_pod_millicores',
    label: 'CPU p95/pod',
    width: 'minmax(100px, 1fr)',
    numeric: true,
    sortKey: 'cpu_p95',
    render: (r) => fmtCores(r.cpu_p95_per_pod_millicores as number),
  },
  {
    key: 'cpu_request_avg_millicores',
    label: 'CPU req',
    width: 'minmax(95px, 1fr)',
    numeric: true,
    sortKey: 'cpu_request_avg',
    render: (r) => fmtCores(r.cpu_request_avg_millicores as number),
  },
  {
    key: 'cpu_waste_millicores',
    label: 'CPU waste',
    width: 'minmax(95px, 1fr)',
    numeric: true,
    sortKey: 'cpu_waste',
    render: (r) => fmtCores(r.cpu_waste_millicores as number),
  },
  {
    key: 'cpu_throttled_max_ratio',
    label: 'Throttle max',
    width: 'minmax(95px, 1fr)',
    numeric: true,
    sortKey: 'cpu_throttled_max',
    render: (r) => fmtPercent(r.cpu_throttled_max_ratio as number),
  },
  {
    key: 'mem_usage_avg_bytes',
    label: 'Mem avg',
    width: 'minmax(90px, 1fr)',
    numeric: true,
    sortKey: 'mem_usage_avg',
    render: (r) => fmtBytes(r.mem_usage_avg_bytes as number),
  },
  {
    key: 'mem_request_avg_bytes',
    label: 'Mem req',
    width: 'minmax(90px, 1fr)',
    numeric: true,
    sortKey: 'mem_request_avg',
    render: (r) => fmtBytes(r.mem_request_avg_bytes as number),
  },
  {
    key: 'mem_waste_bytes',
    label: 'Mem waste',
    width: 'minmax(95px, 1fr)',
    numeric: true,
    sortKey: 'mem_waste',
    render: (r) => fmtBytes(r.mem_waste_bytes as number),
  },
  {
    key: 'psi_cpu_max_ratio',
    label: 'PSI cpu',
    width: 'minmax(75px, 1fr)',
    numeric: true,
    sortKey: 'psi_cpu_max',
    render: (r) => fmtPercent(r.psi_cpu_max_ratio as number),
  },
]

const ROW_KEYS = COLUMNS.map((c) => c.key).concat(['workload_kind'])

export function Workloads({ range }: { range: TimeRange }) {
  const [sortBy, setSortBy] = useState('cpu_waste')
  const [order, setOrder] = useState<'asc' | 'desc'>('desc')
  const [namespace, setNamespace] = useState('')

  const namespaces = useApi(
    () => api.namespaces(range, { sort_by: 'namespace', order: 'asc', limit: 500 }),
    [range.from, range.to],
  )
  const { data, error, loading } = useApi(
    () =>
      api.workloads(range, {
        sort_by: sortBy,
        order,
        limit: 500,
        namespace: namespace || undefined,
      }),
    [range.from, range.to, sortBy, order, namespace],
  )

  const onSort = (key: string) => {
    if (key === sortBy) setOrder(order === 'desc' ? 'asc' : 'desc')
    else {
      setSortBy(key)
      setOrder('desc')
    }
  }

  if (error) return <div className="error">{error}</div>
  const rows = data ? toRows(data, ROW_KEYS) : []

  return (
    <div className="card">
      <h3>Workloads — waste, throttling &amp; pressure</h3>
      <div className="table-toolbar">
        <select value={namespace} onChange={(e) => setNamespace(e.target.value)}>
          <option value="">All namespaces</option>
          {namespaces.data?.namespace.map((ns) => (
            <option key={ns} value={ns}>
              {ns}
            </option>
          ))}
        </select>
        <span className="dim">
          Waste = requested − actually used (average over the selected range)
        </span>
      </div>
      {loading && !data ? (
        <div className="loading">Loading…</div>
      ) : (
        <DataTable columns={COLUMNS} rows={rows} sortBy={sortBy} order={order} onSort={onSort} />
      )}
    </div>
  )
}
