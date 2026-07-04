// Typed client for the ekokube-server columnar API.

export interface TimeRange {
  from: number
  to: number
}

export interface OverviewResponse {
  step: number
  ts: number[]
  cpu_usage_millicores: number[]
  cpu_request_millicores: number[]
  mem_working_set_bytes: number[]
  mem_request_bytes: number[]
  node_ts: number[]
  cpu_used_millicores: number[]
  cpu_capacity_millicores: number[]
  mem_used_bytes: number[]
  mem_total_bytes: number[]
  nodes: number
  pods: number
  namespaces: number
}

export interface NamespacesResponse {
  namespace: string[]
  pods: number[]
  cpu_usage_avg_millicores: number[]
  cpu_request_avg_millicores: number[]
  cpu_waste_millicores: number[]
  cpu_core_hours: number[]
  mem_usage_avg_bytes: number[]
  mem_request_avg_bytes: number[]
  mem_waste_bytes: number[]
  mem_gib_hours: number[]
  has_more: boolean
}

export interface WorkloadsResponse {
  namespace: string[]
  workload_kind: string[]
  workload_name: string[]
  pods: number[]
  cpu_usage_avg_millicores: number[]
  cpu_request_avg_millicores: number[]
  cpu_waste_millicores: number[]
  cpu_p95_per_pod_millicores: number[]
  cpu_throttled_max_ratio: number[]
  mem_usage_avg_bytes: number[]
  mem_request_avg_bytes: number[]
  mem_waste_bytes: number[]
  mem_max_per_pod_bytes: number[]
  psi_cpu_max_ratio: number[]
  psi_mem_max_ratio: number[]
  has_more: boolean
}

export interface NodesResponse {
  node: string[]
  pods: number[]
  cpu_used_avg_millicores: number[]
  cpu_used_max_millicores: number[]
  cpu_capacity_millicores: number[]
  cpu_allocatable_millicores: number[]
  cpu_requested_avg_millicores: number[]
  mem_used_avg_bytes: number[]
  mem_used_max_bytes: number[]
  mem_total_bytes: number[]
  mem_allocatable_bytes: number[]
  mem_requested_avg_bytes: number[]
}

export interface QueryOpts {
  namespace?: string
  workload?: string
  entity?: 'namespace' | 'workload'
  sort_by?: string
  order?: 'asc' | 'desc'
  limit?: number
  offset?: number
}

async function get<T>(path: string, range: TimeRange, opts: QueryOpts = {}): Promise<T> {
  const params = new URLSearchParams({ from: String(range.from), to: String(range.to) })
  for (const [k, v] of Object.entries(opts)) {
    if (v !== undefined && v !== '') params.set(k, String(v))
  }
  const res = await fetch(`${path}?${params}`)
  if (!res.ok) throw new Error(`${path}: ${res.status} ${await res.text()}`)
  return res.json()
}

export type MatcherType = 'namespace_exact' | 'namespace_regex' | 'namespace_label' | 'pod_label'
export type TenantSource = 'static' | 'label_value'

export interface TenantRule {
  matcher_type: MatcherType
  match_key: string
  match_value: string
  tenant_source: TenantSource
  tenant_name: string
}

export interface TenantRow {
  tenant: string
  namespaces: number
  pods: number
  cpu_core_hours: number
  cpu_request_core_hours: number
  mem_gib_hours: number
  mem_request_gib_hours: number
  cost_requested: number
  cost_used: number
}

export interface TenantStat {
  tenant: string
  namespaces: number
  pods: number
  cpu_core_hours: number
  cpu_request_core_hours: number
  mem_gib_hours: number
  mem_request_gib_hours: number
}

export interface Costs {
  cost_core_hour: number
  cost_gib_hour: number
}

async function send<T>(path: string, method: string, body: unknown, range?: TimeRange): Promise<T> {
  const params = range ? `?${new URLSearchParams({ from: String(range.from), to: String(range.to) })}` : ''
  const res = await fetch(`${path}${params}`, {
    method,
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!res.ok) throw new Error(`${path}: ${res.status} ${await res.text()}`)
  return res.json()
}

export function tenantsCsvUrl(range: TimeRange, costs: Costs): string {
  const params = new URLSearchParams({
    from: String(range.from),
    to: String(range.to),
    cost_core_hour: String(costs.cost_core_hour),
    cost_gib_hour: String(costs.cost_gib_hour),
  })
  return `/api/tenants/export.csv?${params}`
}

export interface TimeseriesResponse {
  step: number
  ts: number[]
  cpu_usage_millicores: number[]
  cpu_request_millicores: number[]
  cpu_limit_millicores: number[]
  mem_working_set_bytes: number[]
  mem_request_bytes: number[]
  mem_limit_bytes: number[]
}

export interface PodsResponse {
  pod_name: string[]
  node: string[]
  qos: string[]
  runtime_secs: number[]
  cpu_usage_avg_millicores: number[]
  cpu_p95_millicores: number[]
  cpu_request_millicores: number[]
  cpu_limit_millicores: number[]
  cpu_throttled_max_ratio: number[]
  mem_usage_avg_bytes: number[]
  mem_max_bytes: number[]
  mem_request_bytes: number[]
  psi_cpu_max_ratio: number[]
  psi_mem_max_ratio: number[]
  has_more: boolean
}

export interface SparklineSeries {
  key: string
  ts: number[]
  cpu_millicores: number[]
}

export const api = {
  overview: (r: TimeRange) => get<OverviewResponse>('/api/overview', r),
  timeseries: (r: TimeRange, o?: QueryOpts) => get<TimeseriesResponse>('/api/timeseries', r, o),
  pods: (r: TimeRange, o?: QueryOpts) => get<PodsResponse>('/api/pods', r, o),
  sparklines: (r: TimeRange, o?: QueryOpts) => get<SparklineSeries[]>('/api/sparklines', r, o),
  namespaces: (r: TimeRange, o?: QueryOpts) => get<NamespacesResponse>('/api/namespaces', r, o),
  workloads: (r: TimeRange, o?: QueryOpts) => get<WorkloadsResponse>('/api/workloads', r, o),
  nodes: (r: TimeRange) => get<NodesResponse>('/api/nodes', r),
  tenants: (r: TimeRange, c: Costs) =>
    get<TenantRow[]>('/api/tenants', r, {
      cost_core_hour: String(c.cost_core_hour),
      cost_gib_hour: String(c.cost_gib_hour),
    } as unknown as QueryOpts),
  getRules: async (): Promise<TenantRule[]> => {
    const res = await fetch('/api/rules')
    if (!res.ok) throw new Error(`rules: ${res.status}`)
    return res.json()
  },
  putRules: (rules: TenantRule[]) => send<TenantRule[]>('/api/rules', 'PUT', rules),
  previewRules: (rules: TenantRule[], r: TimeRange) =>
    send<TenantStat[]>('/api/rules/preview', 'POST', rules, r),
}

/** Transposes a columnar response into row objects for table rendering. */
export function toRows(data: object, keys: string[]): Record<string, unknown>[] {
  const columns = data as Record<string, unknown[]>
  const first = columns[keys[0]]
  if (!first) return []
  return first.map((_, i) => {
    const row: Record<string, unknown> = {}
    for (const k of keys) row[k] = columns[k][i]
    return row
  })
}
