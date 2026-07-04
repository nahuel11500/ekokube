import { useEffect, useMemo, useRef, useState } from 'react'
import { PRESETS, RangePicker, type RangePreset } from './components/RangePicker'
import { navigate, routeHref, setRouteParam, useRoute } from './router'
import { updateSettings, useSettings } from './settings'
import { Overview } from './views/Overview'
import { Namespaces } from './views/Namespaces'
import { Workloads } from './views/Workloads'
import { Nodes } from './views/Nodes'
import { Tenants } from './views/Tenants'
import { NamespaceDetail } from './views/NamespaceDetail'
import { WorkloadDetail } from './views/WorkloadDetail'

const VIEWS = [
  { seg: 'overview', label: 'Overview' },
  { seg: 'namespaces', label: 'Namespaces' },
  { seg: 'workloads', label: 'Workloads' },
  { seg: 'tenants', label: 'Tenants' },
  { seg: 'nodes', label: 'Nodes' },
]

function SettingsPopover() {
  const settings = useSettings()
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [open])
  return (
    <div className="popover-anchor" ref={ref}>
      <button className="icon-btn" onClick={() => setOpen(!open)} title="Settings">
        ⚙ Settings
      </button>
      {open && (
        <div className="popover">
          <label>
            Theme
            <select
              value={settings.theme}
              onChange={(e) => updateSettings({ theme: e.target.value as typeof settings.theme })}
            >
              <option value="auto">auto</option>
              <option value="light">light</option>
              <option value="dark">dark</option>
            </select>
          </label>
          <label>
            €/core-hour
            <input
              type="number"
              step="0.001"
              value={settings.costCoreHour}
              onChange={(e) => updateSettings({ costCoreHour: Number(e.target.value) })}
            />
          </label>
          <label>
            €/GiB-hour
            <input
              type="number"
              step="0.001"
              value={settings.costGibHour}
              onChange={(e) => updateSettings({ costGibHour: Number(e.target.value) })}
            />
          </label>
        </div>
      )}
    </div>
  )
}

function Crumbs({ segments }: { segments: string[] }) {
  if (segments.length < 2) return null
  const crumbs: { label: string; href?: string }[] = []
  if (segments[0] === 'namespaces') {
    crumbs.push({ label: 'Namespaces', href: routeHref(['namespaces']) })
    if (segments[1]) {
      const ns = segments[1]
      if (segments[2] === 'workloads' && segments[3]) {
        crumbs.push({ label: ns, href: routeHref(['namespaces', ns]) })
        crumbs.push({ label: segments[3] })
      } else {
        crumbs.push({ label: ns })
      }
    }
  }
  if (crumbs.length === 0) return null
  return (
    <nav className="crumbs" aria-label="Breadcrumb">
      {crumbs.map((c, i) => (
        <span key={i} style={{ display: 'inline-flex', gap: 6 }}>
          {i > 0 && <span className="sep">/</span>}
          {c.href ? <a href={c.href}>{c.label}</a> : <span className="here">{c.label}</span>}
        </span>
      ))}
    </nav>
  )
}

export default function App() {
  const route = useRoute()
  useSettings() // re-render on theme change so charts pick up new CSS vars

  const preset: RangePreset =
    PRESETS.find((p) => p.label === route.params.get('range')) ?? PRESETS[2]

  // Snapping `to` to the minute keeps queries cacheable while staying near-live.
  const range = useMemo(() => {
    const to = Math.floor(Date.now() / 60000) * 60
    return { from: to - preset.seconds, to }
  }, [preset])

  const view = route.segments[0]
  const ns = route.segments[0] === 'namespaces' ? route.segments[1] : undefined
  const workload = ns && route.segments[2] === 'workloads' ? route.segments[3] : undefined

  return (
    <>
      <header className="app-header">
        <a className="app-title" href={routeHref(['overview'])} style={{ textDecoration: 'none' }}>
          eko<span>kube</span>
        </a>
        <nav className="nav">
          {VIEWS.map((v) => (
            <button
              key={v.seg}
              className={v.seg === view ? 'active' : ''}
              onClick={() => navigate([v.seg])}
            >
              {v.label}
            </button>
          ))}
        </nav>
        <Crumbs segments={route.segments} />
        <div className="spacer" />
        <RangePicker selected={preset} onSelect={(p) => setRouteParam('range', p.label)} />
        <SettingsPopover />
      </header>
      <main className="main">
        {view === 'overview' && <Overview range={range} />}
        {view === 'namespaces' && !ns && <Namespaces range={range} />}
        {view === 'namespaces' && ns && !workload && <NamespaceDetail range={range} ns={ns} />}
        {view === 'namespaces' && ns && workload && (
          <WorkloadDetail range={range} ns={ns} workload={workload} />
        )}
        {view === 'workloads' && <Workloads range={range} />}
        {view === 'tenants' && <Tenants range={range} />}
        {view === 'nodes' && <Nodes range={range} />}
      </main>
    </>
  )
}
