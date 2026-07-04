import { useMemo, useState } from 'react'
import { api, toRows, type TimeRange } from '../api'
import { useApi } from '../hooks'
import { fmtBytes, fmtCores, fmtHours } from '../format'
import { navigate, routeHref } from '../router'
import { Sparkline } from '../components/Sparkline'
import { DataTable, type Column } from '../components/DataTable'

function efficiencyBar(usage: number, request: number) {
  const ratio = request > 0 ? Math.min(usage / request, 1) : 0
  return (
    <span className="mini-bar" title={`${(ratio * 100).toFixed(0)}% of requests used`}>
      <i style={{ width: `${ratio * 100}%` }} />
    </span>
  )
}

export function Namespaces({ range }: { range: TimeRange }) {
  const [sortBy, setSortBy] = useState('cpu_usage_avg')
  const [order, setOrder] = useState<'asc' | 'desc'>('desc')

  const { data, error, loading } = useApi(
    () => api.namespaces(range, { sort_by: sortBy, order, limit: 500 }),
    [range.from, range.to, sortBy, order],
  )
  const sparks = useApi(() => api.sparklines(range), [range.from, range.to])
  const sparkByNs = useMemo(
    () => new Map((sparks.data ?? []).map((s) => [s.key, s.cpu_millicores])),
    [sparks.data],
  )

  const columns: Column[] = useMemo(
    () => [
      {
        key: 'namespace',
        label: 'Namespace',
        width: 'minmax(150px, 1.4fr)',
        sortKey: 'namespace',
        render: (r) => <a href={routeHref(['namespaces', String(r.namespace)])}>{String(r.namespace)}</a>,
      },
      {
        key: 'trend',
        label: 'CPU trend',
        width: '110px',
        render: (r) => <Sparkline values={sparkByNs.get(String(r.namespace)) ?? []} />,
      },
      { key: 'pods', label: 'Pods', width: '70px', numeric: true, sortKey: 'pods' },
      {
        key: 'cpu_usage_avg_millicores',
        label: 'CPU avg',
        width: 'minmax(100px, 1fr)',
        numeric: true,
        sortKey: 'cpu_usage_avg',
        render: (r) => fmtCores(r.cpu_usage_avg_millicores as number),
      },
      {
        key: 'cpu_request_avg_millicores',
        label: 'CPU req',
        width: 'minmax(140px, 1fr)',
        numeric: true,
        sortKey: 'cpu_request_avg',
        render: (r) => (
          <>
            {fmtCores(r.cpu_request_avg_millicores as number)}
            {efficiencyBar(
              r.cpu_usage_avg_millicores as number,
              r.cpu_request_avg_millicores as number,
            )}
          </>
        ),
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
        key: 'cpu_core_hours',
        label: 'Core-hours',
        width: 'minmax(90px, 1fr)',
        numeric: true,
        sortKey: 'cpu_core_hours',
        render: (r) => fmtHours(r.cpu_core_hours as number),
      },
      {
        key: 'mem_usage_avg_bytes',
        label: 'Mem avg',
        width: 'minmax(95px, 1fr)',
        numeric: true,
        sortKey: 'mem_usage_avg',
        render: (r) => fmtBytes(r.mem_usage_avg_bytes as number),
      },
      {
        key: 'mem_request_avg_bytes',
        label: 'Mem req',
        width: 'minmax(140px, 1fr)',
        numeric: true,
        sortKey: 'mem_request_avg',
        render: (r) => (
          <>
            {fmtBytes(r.mem_request_avg_bytes as number)}
            {efficiencyBar(r.mem_usage_avg_bytes as number, r.mem_request_avg_bytes as number)}
          </>
        ),
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
        key: 'mem_gib_hours',
        label: 'GiB-hours',
        width: 'minmax(90px, 1fr)',
        numeric: true,
        sortKey: 'mem_gib_hours',
        render: (r) => fmtHours(r.mem_gib_hours as number),
      },
    ],
    [sparkByNs],
  )

  const onSort = (key: string) => {
    if (key === sortBy) setOrder(order === 'desc' ? 'asc' : 'desc')
    else {
      setSortBy(key)
      setOrder('desc')
    }
  }

  if (error) return <div className="error">{error}</div>

  const rows = data
    ? toRows(data, [
        'namespace',
        'pods',
        'cpu_usage_avg_millicores',
        'cpu_request_avg_millicores',
        'cpu_waste_millicores',
        'cpu_core_hours',
        'mem_usage_avg_bytes',
        'mem_request_avg_bytes',
        'mem_waste_bytes',
        'mem_gib_hours',
      ])
    : []

  return (
    <div className="card">
      <h3>Namespaces — consumption &amp; waste</h3>
      {loading && !data ? (
        <div className="skeleton" style={{ height: 240 }} />
      ) : (
        <DataTable
          columns={columns}
          rows={rows}
          sortBy={sortBy}
          order={order}
          onSort={onSort}
          onRowClick={(r) => navigate(['namespaces', String(r.namespace)])}
        />
      )}
    </div>
  )
}
