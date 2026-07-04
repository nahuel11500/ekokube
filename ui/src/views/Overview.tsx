import { useMemo } from 'react'
import { api, type TimeRange } from '../api'
import { useApi } from '../hooks'
import { fmtCount } from '../format'
import { TimeSeriesChart, type ChartSeries } from '../components/TimeSeriesChart'

/** Joins two bucket-aligned series sets on their union of timestamps. */
function mergeOnTs(
  aTs: number[],
  a: (number | null)[][],
  bTs: number[],
  b: (number | null)[][],
): { ts: number[]; merged: (number | null)[][] } {
  const union = [...new Set([...aTs, ...bTs])].sort((x, y) => x - y)
  const aIdx = new Map(aTs.map((t, i) => [t, i]))
  const bIdx = new Map(bTs.map((t, i) => [t, i]))
  const merged = [
    ...a.map((col) => union.map((t) => (aIdx.has(t) ? col[aIdx.get(t)!] : null))),
    ...b.map((col) => union.map((t) => (bIdx.has(t) ? col[bIdx.get(t)!] : null))),
  ]
  return { ts: union, merged }
}

export function Overview({ range }: { range: TimeRange }) {
  const { data, error, loading } = useApi(() => api.overview(range), [range.from, range.to])

  const cpu = useMemo(() => {
    if (!data) return null
    const { ts, merged } = mergeOnTs(
      data.ts,
      [data.cpu_usage_millicores, data.cpu_request_millicores],
      data.node_ts,
      [data.cpu_capacity_millicores],
    )
    const series: ChartSeries[] = [
      { label: 'Usage', values: merged[0], colorVar: '--series-1', fill: true },
      { label: 'Requests', values: merged[1], colorVar: '--series-2' },
      { label: 'Capacity', values: merged[2], colorVar: '--text-muted', dash: [6, 4] },
    ]
    return { ts, series }
  }, [data])

  const mem = useMemo(() => {
    if (!data) return null
    const { ts, merged } = mergeOnTs(
      data.ts,
      [data.mem_working_set_bytes, data.mem_request_bytes],
      data.node_ts,
      [data.mem_total_bytes],
    )
    const series: ChartSeries[] = [
      { label: 'Working set', values: merged[0], colorVar: '--series-1', fill: true },
      { label: 'Requests', values: merged[1], colorVar: '--series-2' },
      { label: 'Total', values: merged[2], colorVar: '--text-muted', dash: [6, 4] },
    ]
    return { ts, series }
  }, [data])

  if (error) return <div className="error">{error}</div>
  if (loading || !data) return <div className="loading">Loading…</div>

  return (
    <>
      <div className="tiles">
        <div className="tile">
          <div className="label">Nodes</div>
          <div className="value">{fmtCount(data.nodes)}</div>
        </div>
        <div className="tile">
          <div className="label">Pods</div>
          <div className="value">{fmtCount(data.pods)}</div>
        </div>
        <div className="tile">
          <div className="label">Namespaces</div>
          <div className="value">{fmtCount(data.namespaces)}</div>
        </div>
      </div>
      <div className="charts-row">
        <div className="card">
          <h3>CPU — usage vs requests vs capacity</h3>
          {cpu && <TimeSeriesChart ts={cpu.ts} series={cpu.series} unit="cores" />}
        </div>
        <div className="card">
          <h3>Memory — working set vs requests vs total</h3>
          {mem && <TimeSeriesChart ts={mem.ts} series={mem.series} unit="bytes" />}
        </div>
      </div>
    </>
  )
}
