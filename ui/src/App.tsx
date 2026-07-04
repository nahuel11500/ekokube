import { useMemo, useState } from 'react'
import { PRESETS, RangePicker, type RangePreset } from './components/RangePicker'
import { Overview } from './views/Overview'
import { Namespaces } from './views/Namespaces'
import { Workloads } from './views/Workloads'
import { Nodes } from './views/Nodes'
import { Tenants } from './views/Tenants'

const VIEWS = ['Overview', 'Namespaces', 'Workloads', 'Tenants', 'Nodes'] as const
type View = (typeof VIEWS)[number]

function viewFromHash(): View {
  const h = window.location.hash.replace('#', '')
  return (VIEWS as readonly string[]).includes(h) ? (h as View) : 'Overview'
}

export default function App() {
  const [view, setViewState] = useState<View>(viewFromHash)
  const [preset, setPreset] = useState<RangePreset>(PRESETS[2]) // 24h
  const setView = (v: View) => {
    window.location.hash = v
    setViewState(v)
  }

  // Recomputed when the preset changes; snapping `to` to the minute keeps the
  // query cacheable while staying near-live.
  const range = useMemo(() => {
    const to = Math.floor(Date.now() / 60000) * 60
    return { from: to - preset.seconds, to }
  }, [preset])

  return (
    <>
      <header className="app-header">
        <div className="app-title">
          eko<span>kube</span>
        </div>
        <nav className="nav">
          {VIEWS.map((v) => (
            <button key={v} className={v === view ? 'active' : ''} onClick={() => setView(v)}>
              {v}
            </button>
          ))}
        </nav>
        <div className="spacer" />
        <RangePicker selected={preset} onSelect={setPreset} />
      </header>
      <main className="main">
        {view === 'Overview' && <Overview range={range} />}
        {view === 'Namespaces' && <Namespaces range={range} />}
        {view === 'Workloads' && <Workloads range={range} />}
        {view === 'Tenants' && <Tenants range={range} />}
        {view === 'Nodes' && <Nodes range={range} />}
      </main>
    </>
  )
}
