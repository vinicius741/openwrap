import React from 'react'
import ReactDOM from 'react-dom/client'

import { App } from './app/App'
import { initTheme } from './lib/theme'
import './styles/global.css'

initTheme()

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
