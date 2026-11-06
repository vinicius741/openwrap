import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { fileURLToPath } from 'node:url'

export default defineConfig({
  plugins: [
    react(),
    {
      name: 'fontsource-woff2-only',
      enforce: 'pre',
      transform(source, id) {
        if (!id.includes('/@fontsource/') || !id.endsWith('.css')) return
        // macOS WebKit supports WOFF2. Keep every unicode range and weight,
        // without emitting Fontsource's redundant WOFF fallback assets.
        return source.replace(/,\s*url\([^)]*\.woff\)\s*format\(['"]woff['"]\)/g, '')
      },
    },
  ],
  resolve: {
    alias: {
      react: fileURLToPath(new URL('./node_modules/react', import.meta.url)),
      'react-dom': fileURLToPath(new URL('./node_modules/react-dom', import.meta.url)),
    },
  },
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true,
  },
})
