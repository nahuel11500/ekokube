// App-level settings shared across views (persisted in localStorage):
// theme + billing rates.

import { useSyncExternalStore } from 'react'

export interface Settings {
  theme: 'auto' | 'light' | 'dark'
  costCoreHour: number
  costGibHour: number
}

const KEY = 'ekokube-settings'
let current: Settings = load()
const listeners = new Set<() => void>()

function load(): Settings {
  try {
    const stored = JSON.parse(localStorage.getItem(KEY) ?? '{}')
    return {
      theme: stored.theme ?? 'auto',
      costCoreHour: Number(stored.costCoreHour ?? 0.05),
      costGibHour: Number(stored.costGibHour ?? 0.01),
    }
  } catch {
    return { theme: 'auto', costCoreHour: 0.05, costGibHour: 0.01 }
  }
}

export function applyTheme(theme: Settings['theme']) {
  const root = document.documentElement
  if (theme === 'auto') delete root.dataset.theme
  else root.dataset.theme = theme
}

export function updateSettings(patch: Partial<Settings>) {
  current = { ...current, ...patch }
  localStorage.setItem(KEY, JSON.stringify(current))
  applyTheme(current.theme)
  listeners.forEach((l) => l())
}

export function useSettings(): Settings {
  return useSyncExternalStore(
    (cb) => {
      listeners.add(cb)
      return () => listeners.delete(cb)
    },
    () => current,
  )
}

applyTheme(current.theme)
