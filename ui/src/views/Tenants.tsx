// Chargeback: consumption per tenant under a configurable rule set.
// Rules are evaluated at query time, so edits apply retroactively; the
// preview panel evaluates the draft rule set without saving it.

import { useEffect, useMemo, useState } from 'react'
import {
  api,
  tenantsCsvUrl,
  type Costs,
  type TenantRule,
  type TenantStat,
  type TimeRange,
} from '../api'
import { useApi } from '../hooks'
import { fmtHours } from '../format'
import { DataTable, type Column } from '../components/DataTable'

const EMPTY_RULE: TenantRule = {
  matcher_type: 'namespace_label',
  match_key: '',
  match_value: '',
  tenant_source: 'label_value',
  tenant_name: '',
}

function loadCosts(): Costs {
  try {
    const stored = localStorage.getItem('ekokube-costs')
    if (stored) return JSON.parse(stored)
  } catch {
    /* fall through */
  }
  return { cost_core_hour: 0.05, cost_gib_hour: 0.01 }
}

const TENANT_COLUMNS: Column[] = [
  { key: 'tenant', label: 'Tenant', width: 'minmax(140px, 1.4fr)' },
  { key: 'namespaces', label: 'Namespaces', width: '90px', numeric: true },
  { key: 'pods', label: 'Pods', width: '70px', numeric: true },
  {
    key: 'cpu_core_hours',
    label: 'Core-hours used',
    width: 'minmax(120px, 1fr)',
    numeric: true,
    render: (r) => fmtHours(r.cpu_core_hours as number),
  },
  {
    key: 'cpu_request_core_hours',
    label: 'Core-hours reserved',
    width: 'minmax(135px, 1fr)',
    numeric: true,
    render: (r) => fmtHours(r.cpu_request_core_hours as number),
  },
  {
    key: 'mem_gib_hours',
    label: 'GiB-hours used',
    width: 'minmax(115px, 1fr)',
    numeric: true,
    render: (r) => fmtHours(r.mem_gib_hours as number),
  },
  {
    key: 'mem_request_gib_hours',
    label: 'GiB-hours reserved',
    width: 'minmax(130px, 1fr)',
    numeric: true,
    render: (r) => fmtHours(r.mem_request_gib_hours as number),
  },
  {
    key: 'cost_requested',
    label: 'Cost (reserved)',
    width: 'minmax(110px, 1fr)',
    numeric: true,
    render: (r) => `€${(r.cost_requested as number).toFixed(2)}`,
  },
  {
    key: 'cost_used',
    label: 'Cost (used)',
    width: 'minmax(100px, 1fr)',
    numeric: true,
    render: (r) => `€${(r.cost_used as number).toFixed(2)}`,
  },
]

function RuleEditor({
  rule,
  onChange,
  onRemove,
  onMove,
}: {
  rule: TenantRule
  onChange: (r: TenantRule) => void
  onRemove: () => void
  onMove: (dir: -1 | 1) => void
}) {
  const isLabel = rule.matcher_type === 'namespace_label' || rule.matcher_type === 'pod_label'
  return (
    <div className="rule-row">
      <span className="rule-move">
        <button onClick={() => onMove(-1)} title="Move up">↑</button>
        <button onClick={() => onMove(1)} title="Move down">↓</button>
      </span>
      <select
        value={rule.matcher_type}
        onChange={(e) => {
          const matcher_type = e.target.value as TenantRule['matcher_type']
          const labelType = matcher_type === 'namespace_label' || matcher_type === 'pod_label'
          onChange({
            ...rule,
            matcher_type,
            tenant_source: labelType ? rule.tenant_source : 'static',
          })
        }}
      >
        <option value="namespace_exact">Namespace is</option>
        <option value="namespace_regex">Namespace matches regex</option>
        <option value="namespace_label">Namespace label</option>
        <option value="pod_label">Pod label</option>
      </select>
      {isLabel && (
        <input
          placeholder="label key (e.g. tenant)"
          value={rule.match_key}
          onChange={(e) => onChange({ ...rule, match_key: e.target.value })}
        />
      )}
      <input
        placeholder={
          rule.matcher_type === 'namespace_exact'
            ? 'namespace name'
            : rule.matcher_type === 'namespace_regex'
              ? 'regex (e.g. ^team-)'
              : 'label value (empty = any)'
        }
        value={rule.match_value}
        onChange={(e) => onChange({ ...rule, match_value: e.target.value })}
      />
      <span className="dim">→</span>
      {isLabel ? (
        <select
          value={rule.tenant_source}
          onChange={(e) =>
            onChange({ ...rule, tenant_source: e.target.value as TenantRule['tenant_source'] })
          }
        >
          <option value="label_value">tenant = label value</option>
          <option value="static">fixed tenant name</option>
        </select>
      ) : (
        <span className="dim">tenant</span>
      )}
      {rule.tenant_source === 'static' && (
        <input
          placeholder="tenant name"
          value={rule.tenant_name}
          onChange={(e) => onChange({ ...rule, tenant_name: e.target.value })}
        />
      )}
      <button className="rule-remove" onClick={onRemove} title="Remove rule">✕</button>
    </div>
  )
}

export function Tenants({ range }: { range: TimeRange }) {
  const [costs, setCosts] = useState<Costs>(loadCosts)
  const [rules, setRules] = useState<TenantRule[] | null>(null)
  const [dirty, setDirty] = useState(false)
  const [preview, setPreview] = useState<TenantStat[] | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)

  useEffect(() => {
    localStorage.setItem('ekokube-costs', JSON.stringify(costs))
  }, [costs])

  useEffect(() => {
    api.getRules().then(setRules).catch((e) => setSaveError(String(e)))
  }, [])

  const tenants = useApi(
    () => api.tenants(range, costs),
    [range.from, range.to, costs.cost_core_hour, costs.cost_gib_hour, dirty],
  )

  // Debounced live preview of the draft rule set.
  useEffect(() => {
    if (!rules || !dirty) return
    const timer = setTimeout(() => {
      api
        .previewRules(rules, range)
        .then((p) => {
          setPreview(p)
          setSaveError(null)
        })
        .catch((e) => setSaveError(String(e)))
    }, 500)
    return () => clearTimeout(timer)
  }, [rules, dirty, range])

  const editRules = (next: TenantRule[]) => {
    setRules(next)
    setDirty(true)
  }

  const save = async () => {
    if (!rules) return
    try {
      await api.putRules(rules)
      setDirty(false)
      setPreview(null)
      setSaveError(null)
    } catch (e) {
      setSaveError(String(e))
    }
  }

  const rows = useMemo(
    () => (tenants.data ?? []) as unknown as Record<string, unknown>[],
    [tenants.data],
  )

  return (
    <>
      <div className="card">
        <h3>Tenants — consumption &amp; chargeback</h3>
        <div className="table-toolbar">
          <label>
            €/core-hour{' '}
            <input
              type="number"
              step="0.001"
              style={{ width: 80 }}
              value={costs.cost_core_hour}
              onChange={(e) => setCosts({ ...costs, cost_core_hour: Number(e.target.value) })}
            />
          </label>
          <label>
            €/GiB-hour{' '}
            <input
              type="number"
              step="0.001"
              style={{ width: 80 }}
              value={costs.cost_gib_hour}
              onChange={(e) => setCosts({ ...costs, cost_gib_hour: Number(e.target.value) })}
            />
          </label>
          <div className="spacer" />
          <a className="btn" href={tenantsCsvUrl(range, costs)} download>
            Export CSV
          </a>
        </div>
        {tenants.error && <div className="error">{tenants.error}</div>}
        {tenants.loading && !tenants.data ? (
          <div className="loading">Loading…</div>
        ) : (
          <DataTable
            columns={TENANT_COLUMNS}
            rows={rows}
            sortBy=""
            order="desc"
            onSort={() => {}}
          />
        )}
      </div>

      <div className="card">
        <h3>Tenant rules — first match wins, applied retroactively</h3>
        {rules === null ? (
          <div className="loading">Loading…</div>
        ) : (
          <>
            {rules.map((rule, i) => (
              <RuleEditor
                key={i}
                rule={rule}
                onChange={(r) => editRules(rules.map((x, j) => (j === i ? r : x)))}
                onRemove={() => editRules(rules.filter((_, j) => j !== i))}
                onMove={(dir) => {
                  const j = i + dir
                  if (j < 0 || j >= rules.length) return
                  const next = [...rules]
                  ;[next[i], next[j]] = [next[j], next[i]]
                  editRules(next)
                }}
              />
            ))}
            <div className="table-toolbar" style={{ marginTop: 10 }}>
              <button className="btn" onClick={() => editRules([...rules, { ...EMPTY_RULE }])}>
                + Add rule
              </button>
              <button className="btn primary" onClick={save} disabled={!dirty}>
                Save rules
              </button>
              {dirty && <span className="dim">unsaved changes — preview below</span>}
            </div>
            {saveError && <div className="error">{saveError}</div>}
            {dirty && preview && (
              <div style={{ marginTop: 10 }}>
                <h3>Preview with draft rules</h3>
                <DataTable
                  columns={TENANT_COLUMNS.slice(0, 7)}
                  rows={preview as unknown as Record<string, unknown>[]}
                  sortBy=""
                  order="desc"
                  onSort={() => {}}
                />
              </div>
            )}
          </>
        )}
      </div>
    </>
  )
}
