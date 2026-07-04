// Workload drill-down: right-sizing recommendation, usage/limits charts,
// per-pod table with throttling and pressure.

import { api, toRows, type TimeRange } from '../api'
import { useApi } from '../hooks'
import { fmtBytes, fmtCores, fmtPercent } from '../format'
import { useSettings } from '../settings'
import { Badge } from '../components/Badge'
import { UsageCharts } from '../components/UsageCharts'
import { DataTable, type Column } from '../components/DataTable'

const POD_COLUMNS: Column[] = [
  { key: 'pod_name', label: 'Pod', width: 'minmax(220px, 1.8fr)' },
  { key: 'node', label: 'Node', width: 'minmax(140px, 1.2fr)' },
  { key: 'qos', label: 'QoS', width: '90px' },
  {
    key: 'runtime_secs',
    label: 'Observed',
    width: '90px',
    numeric: true,
    render: (r) => `${((r.runtime_secs as number) / 3600).toFixed(1)}h`,
  },
  {
    key: 'cpu_usage_avg_millicores',
    label: 'CPU avg',
    width: 'minmax(90px, 1fr)',
    numeric: true,
    render: (r) => fmtCores(r.cpu_usage_avg_millicores as number),
  },
  {
    key: 'cpu_p95_millicores',
    label: 'CPU p95',
    width: 'minmax(90px, 1fr)',
    numeric: true,
    render: (r) => fmtCores(r.cpu_p95_millicores as number),
  },
  {
    key: 'cpu_request_millicores',
    label: 'CPU req',
    width: 'minmax(90px, 1fr)',
    numeric: true,
    render: (r) => fmtCores(r.cpu_request_millicores as number),
  },
  {
    key: 'cpu_throttled_max_ratio',
    label: 'Throttle',
    width: 'minmax(95px, 1fr)',
    numeric: true,
    render: (r) => {
      const v = r.cpu_throttled_max_ratio as number
      return v > 0.25 ? (
        <Badge level="serious" title="max fraction of throttled CFS periods">
          {fmtPercent(v)}
        </Badge>
      ) : (
        fmtPercent(v)
      )
    },
  },
  {
    key: 'mem_usage_avg_bytes',
    label: 'Mem avg',
    width: 'minmax(90px, 1fr)',
    numeric: true,
    render: (r) => fmtBytes(r.mem_usage_avg_bytes as number),
  },
  {
    key: 'mem_max_bytes',
    label: 'Mem max',
    width: 'minmax(90px, 1fr)',
    numeric: true,
    render: (r) => fmtBytes(r.mem_max_bytes as number),
  },
  {
    key: 'psi_cpu_max_ratio',
    label: 'PSI cpu',
    width: 'minmax(80px, 1fr)',
    numeric: true,
    render: (r) => fmtPercent(r.psi_cpu_max_ratio as number),
  },
]

const ROW_KEYS = POD_COLUMNS.map((c) => c.key)

/** Suggested request: p95 across pods with 20% headroom, floored sensibly. */
function recommend(p95: number, floor: number): number {
  return Math.max(Math.ceil((p95 * 1.2) / 10) * 10, floor)
}

export function WorkloadDetail({
  range,
  ns,
  workload,
}: {
  range: TimeRange
  ns: string
  workload: string
}) {
  const settings = useSettings()
  const pods = useApi(
    () => api.pods(range, { namespace: ns, workload, limit: 500, sort_by: 'cpu_usage_avg' }),
    [range.from, range.to, ns, workload],
  )

  const rows = pods.data ? toRows(pods.data, ROW_KEYS) : []

  // Right-sizing from per-pod truth: worst p95 across pods vs current request.
  let reco = null
  if (pods.data && pods.data.pod_name.length > 0) {
    const d = pods.data
    const n = d.pod_name.length
    const cpuP95 = Math.max(...d.cpu_p95_millicores)
    const cpuReq = Math.max(...d.cpu_request_millicores)
    const memMax = Math.max(...d.mem_max_bytes)
    const memReq = Math.max(...d.mem_request_bytes)
    const cpuSuggest = recommend(cpuP95, 10)
    const memSuggest = Math.max(Math.ceil((memMax * 1.2) / 2 ** 20) * 2 ** 20, 16 * 2 ** 20)
    const hoursPerMonth = 730
    const savings =
      (Math.max(cpuReq - cpuSuggest, 0) / 1000) * n * hoursPerMonth * settings.costCoreHour +
      (Math.max(memReq - memSuggest, 0) / 2 ** 30) * n * hoursPerMonth * settings.costGibHour
    reco = { cpuReq, cpuSuggest, memReq, memSuggest, savings, n }
  }

  return (
    <>
      <div className="page-title">
        <h2>{workload}</h2>
        <span className="kind">workload in {ns}</span>
      </div>
      {reco && (
        <div className="reco">
          <span className="head">Right-sizing</span>
          <span className="item">
            CPU request <b>{fmtCores(reco.cpuReq)}</b> → suggested{' '}
            <b>{fmtCores(reco.cpuSuggest)}</b> <span className="dim">(p95 + 20%)</span>
          </span>
          <span className="item">
            Memory request <b>{fmtBytes(reco.memReq)}</b> → suggested{' '}
            <b>{fmtBytes(reco.memSuggest)}</b> <span className="dim">(max + 20%)</span>
          </span>
          {reco.savings > 0.005 && (
            <span className="item">
              est. savings <b>€{reco.savings.toFixed(2)}/month</b>{' '}
              <span className="dim">({reco.n} pods)</span>
            </span>
          )}
        </div>
      )}
      <UsageCharts range={range} namespace={ns} workload={workload} />
      <div className="card">
        <h3>Pods</h3>
        {pods.error && <div className="error">{pods.error}</div>}
        {pods.loading && !pods.data ? (
          <div className="skeleton" style={{ height: 160 }} />
        ) : (
          <DataTable
            columns={POD_COLUMNS}
            rows={rows}
            sortBy="cpu_usage_avg"
            order="desc"
            onSort={() => {}}
          />
        )}
      </div>
    </>
  )
}
