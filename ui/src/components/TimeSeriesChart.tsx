// Canvas time-series chart (uPlot): crosshair + value legend on hover,
// theme-aware colors read from CSS custom properties.

import { useEffect, useRef } from 'react'
import uPlot from 'uplot'
import 'uplot/dist/uPlot.min.css'
import { fmtBytes, fmtCores } from '../format'

export interface ChartSeries {
  label: string
  values: (number | null)[]
  /** CSS variable carrying the stroke color, e.g. '--series-1' */
  colorVar: string
  fill?: boolean
  dash?: number[]
}

interface Props {
  ts: number[]
  series: ChartSeries[]
  unit: 'cores' | 'bytes'
  height?: number
}

function cssColor(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim()
}

function withAlpha(hex: string, alpha: number): string {
  const n = parseInt(hex.slice(1), 16)
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`
}

export function TimeSeriesChart({ ts, series, unit, height = 220 }: Props) {
  const wrapRef = useRef<HTMLDivElement>(null)
  const plotRef = useRef<uPlot | null>(null)

  useEffect(() => {
    const wrap = wrapRef.current
    if (!wrap) return

    const fmt = unit === 'cores' ? fmtCores : fmtBytes
    const axisColor = cssColor('--text-muted')
    const gridColor = cssColor('--gridline')

    const build = () => {
      plotRef.current?.destroy()
      const options: uPlot.Options = {
        width: wrap.clientWidth,
        height,
        cursor: { points: { size: 7 } },
        scales: { x: { time: true } },
        legend: { live: true },
        series: [
          {},
          ...series.map((s) => {
            const color = cssColor(s.colorVar)
            return {
              label: s.label,
              stroke: color,
              width: 2,
              dash: s.dash,
              points: { show: false },
              fill: s.fill ? withAlpha(color, 0.12) : undefined,
              value: (_u: uPlot, v: number | null) => (v == null ? '–' : fmt(v)),
            }
          }),
        ],
        axes: [
          { stroke: axisColor, grid: { stroke: gridColor, width: 1 }, ticks: { show: false } },
          {
            stroke: axisColor,
            grid: { stroke: gridColor, width: 1 },
            ticks: { show: false },
            size: 68,
            values: (_u, splits) => splits.map((v) => fmt(v)),
          },
        ],
      }
      const data = [ts, ...series.map((s) => s.values)] as uPlot.AlignedData
      plotRef.current = new uPlot(options, data, wrap)
    }

    build()
    const resize = new ResizeObserver(() => {
      plotRef.current?.setSize({ width: wrap.clientWidth, height })
    })
    resize.observe(wrap)
    const scheme = window.matchMedia('(prefers-color-scheme: dark)')
    scheme.addEventListener('change', build)
    return () => {
      resize.disconnect()
      scheme.removeEventListener('change', build)
      plotRef.current?.destroy()
      plotRef.current = null
    }
  }, [ts, series, unit, height])

  return <div className="uplot-wrap" ref={wrapRef} />
}
