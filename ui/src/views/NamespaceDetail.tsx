// Namespace drill-down: its usage charts + its workloads (click through to
// workload detail).

import { type TimeRange } from '../api'
import { UsageCharts } from '../components/UsageCharts'
import { WorkloadsTable } from './Workloads'

export function NamespaceDetail({ range, ns }: { range: TimeRange; ns: string }) {
  return (
    <>
      <div className="page-title">
        <h2>{ns}</h2>
        <span className="kind">namespace</span>
      </div>
      <UsageCharts range={range} namespace={ns} />
      <div className="card">
        <h3>Workloads in {ns}</h3>
        <WorkloadsTable range={range} namespace={ns} />
      </div>
    </>
  )
}
