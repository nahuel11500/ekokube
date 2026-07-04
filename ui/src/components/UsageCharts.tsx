// CPU + memory usage/requests/limits chart pair for any scope
// (cluster, namespace, or workload) via /api/timeseries.

import { api, type TimeRange } from '../api'
import { useApi } from '../hooks'
import { TimeSeriesChart, type ChartSeries } from './TimeSeriesChart'

interface Props {
  range: TimeRange
  namespace?: string
  workload?: string
}

export function UsageCharts({ range, namespace, workload }: Props) {
  const { data, error, loading } = useApi(
    () => api.timeseries(range, { namespace, workload }),
    [range.from, range.to, namespace, workload],
  )

  if (error) return <div className="error">{error}</div>
  if (loading || !data)
    return (
      <div className="charts-row">
        <div className="card skeleton" style={{ height: 280 }} />
        <div className="card skeleton" style={{ height: 280 }} />
      </div>
    )

  const cpu: ChartSeries[] = [
    { label: 'Usage', values: data.cpu_usage_millicores, colorVar: '--series-1', fill: true },
    { label: 'Requests', values: data.cpu_request_millicores, colorVar: '--series-2' },
    { label: 'Limits', values: data.cpu_limit_millicores, colorVar: '--series-3', dash: [6, 4] },
  ]
  const mem: ChartSeries[] = [
    { label: 'Working set', values: data.mem_working_set_bytes, colorVar: '--series-1', fill: true },
    { label: 'Requests', values: data.mem_request_bytes, colorVar: '--series-2' },
    { label: 'Limits', values: data.mem_limit_bytes, colorVar: '--series-3', dash: [6, 4] },
  ]
  return (
    <div className="charts-row">
      <div className="card">
        <h3>CPU — usage vs requests vs limits</h3>
        <TimeSeriesChart ts={data.ts} series={cpu} unit="cores" />
      </div>
      <div className="card">
        <h3>Memory — working set vs requests vs limits</h3>
        <TimeSeriesChart ts={data.ts} series={mem} unit="bytes" />
      </div>
    </div>
  )
}
