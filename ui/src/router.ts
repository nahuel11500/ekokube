// Tiny hash router: #/view/seg1/seg2?range=24h — URL-addressable state so
// every screen (incl. drill-downs and the selected range) is a shareable link.

import { useEffect, useState } from 'react'

export interface Route {
  /** path segments, e.g. ['namespaces', 'shop', 'workloads', 'web'] */
  segments: string[]
  params: URLSearchParams
}

function parse(): Route {
  const hash = window.location.hash.replace(/^#\/?/, '')
  const [path, query = ''] = hash.split('?')
  // backward compat with the old '#Overview' style
  const segments = path
    .split('/')
    .filter(Boolean)
    .map((s) => decodeURIComponent(s))
  if (segments.length === 1 && /^[A-Z]/.test(segments[0])) {
    segments[0] = segments[0].toLowerCase()
  }
  if (segments.length === 0) segments.push('overview')
  return { segments, params: new URLSearchParams(query) }
}

export function routeHref(segments: string[], params?: Record<string, string>): string {
  const path = segments.map(encodeURIComponent).join('/')
  const current = parse().params
  const merged = new URLSearchParams()
  // the range is global state: carry it across navigations
  const range = current.get('range')
  if (range) merged.set('range', range)
  for (const [k, v] of Object.entries(params ?? {})) merged.set(k, v)
  const q = merged.toString()
  return `#/${path}${q ? `?${q}` : ''}`
}

export function navigate(segments: string[], params?: Record<string, string>) {
  window.location.hash = routeHref(segments, params)
}

export function setRouteParam(key: string, value: string) {
  const route = parse()
  route.params.set(key, value)
  const q = route.params.toString()
  window.location.hash = `#/${route.segments.map(encodeURIComponent).join('/')}${q ? `?${q}` : ''}`
}

export function useRoute(): Route {
  const [route, setRoute] = useState<Route>(parse)
  useEffect(() => {
    const onChange = () => setRoute(parse())
    window.addEventListener('hashchange', onChange)
    return () => window.removeEventListener('hashchange', onChange)
  }, [])
  return route
}
