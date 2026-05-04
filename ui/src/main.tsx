import React from 'react'
import ReactDOM from 'react-dom/client'

import '@fontsource/jetbrains-mono/400.css'
import '@fontsource/jetbrains-mono/500.css'

import { App } from './app/App'
import { initTheme } from './lib/theme'
import './styles/global.css'

void initTheme().catch((error: unknown) => {
  console.error('Failed to initialize theme', error)
})

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
