export type AppTheme = 'midnight' | 'dawn' | 'ocean'

export const THEMES: { id: AppTheme; label: string }[] = [
  { id: 'midnight', label: 'Midnight' },
  { id: 'dawn', label: 'Dawn' },
  { id: 'ocean', label: 'Ocean' },
]

const STORAGE_KEY = 'openwrap-theme'

export function getStoredTheme(): AppTheme {
  const raw = localStorage.getItem(STORAGE_KEY)
  if (raw === 'dawn' || raw === 'ocean' || raw === 'midnight') {
    return raw
  }
  return 'midnight'
}

export function setStoredTheme(theme: AppTheme): void {
  localStorage.setItem(STORAGE_KEY, theme)
}

export function applyTheme(theme: AppTheme): void {
  document.documentElement.dataset.theme = theme
}

export function initTheme(): AppTheme {
  const theme = getStoredTheme()
  applyTheme(theme)
  return theme
}
