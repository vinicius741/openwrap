export type AppTheme = 'midnight' | 'dawn' | 'ocean'

export const THEMES: { id: AppTheme; label: string; font: string; fontKey: string }[] = [
  { id: 'midnight', label: 'Midnight', font: 'Inter', fontKey: 'inter' },
  { id: 'dawn', label: 'Dawn', font: 'Outfit', fontKey: 'outfit' },
  { id: 'ocean', label: 'Ocean', font: 'IBM Plex Sans', fontKey: 'ibm-plex-sans' },
]

const FONT_LOADERS: Record<AppTheme, () => Promise<unknown[]>> = {
  midnight: () =>
    Promise.all([
      import('@fontsource/inter/400.css'),
      import('@fontsource/inter/500.css'),
      import('@fontsource/inter/600.css'),
      import('@fontsource/inter/700.css'),
    ]),
  dawn: () =>
    Promise.all([
      import('@fontsource/outfit/400.css'),
      import('@fontsource/outfit/500.css'),
      import('@fontsource/outfit/600.css'),
      import('@fontsource/outfit/700.css'),
    ]),
  ocean: () =>
    Promise.all([
      import('@fontsource/ibm-plex-sans/400.css'),
      import('@fontsource/ibm-plex-sans/500.css'),
      import('@fontsource/ibm-plex-sans/600.css'),
      import('@fontsource/ibm-plex-sans/700.css'),
    ]),
}

const loadedThemes = new Set<AppTheme>()

function loadThemeFonts(theme: AppTheme): Promise<void> {
  if (loadedThemes.has(theme)) return Promise.resolve()
  loadedThemes.add(theme)
  return FONT_LOADERS[theme]().then(() => {})
}

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
  void loadThemeFonts(theme)
  document.documentElement.dataset.theme = theme
}

export async function initTheme(): Promise<AppTheme> {
  const theme = getStoredTheme()
  await loadThemeFonts(theme)
  applyTheme(theme)
  return theme
}
