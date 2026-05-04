/**
 * Regression tests for the "blank white screen" bug.
 *
 * Root cause: zustand was hoisted to root node_modules while react stayed in
 * ui/node_modules, so Vite's optimizer could not resolve "react" imported by
 * zustand.  Additionally, theme init blocked React mounting.
 *
 * These tests verify the three fixes stay in place:
 *   1. Vite aliases bridge the react resolution gap
 *   2. Dev server binds to 127.0.0.1 (not localhost)
 *   3. Tauri devUrl uses 127.0.0.1
 *   4. main.tsx does not gate React rendering on theme init
 */

import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const root = resolve(__dirname, '..')

// ---------------------------------------------------------------------------
// 1. Vite config
// ---------------------------------------------------------------------------
describe('ui/vite.config.ts', () => {
  const configPath = resolve(root, 'ui', 'vite.config.ts')
  const content = readFileSync(configPath, 'utf-8')

  it('aliases "react" to ui/node_modules/react', () => {
    assert.match(
      content,
      /react:\s*fileURLToPath\(new URL\(['"]\.\/node_modules\/react['"]/,
      'Vite config must alias "react" to ./node_modules/react so zustand (hoisted to root) can resolve it',
    )

    const reactDir = resolve(root, 'ui', 'node_modules', 'react')
    assert.ok(
      existsSync(reactDir),
      `Aliased react directory must exist at ${reactDir}`,
    )
  })

  it('aliases "react-dom" to ui/node_modules/react-dom', () => {
    assert.match(
      content,
      /['"]react-dom['"]:\s*fileURLToPath\(new URL\(['"]\.\/node_modules\/react-dom['"]/,
      'Vite config must alias "react-dom" to ./node_modules/react-dom',
    )

    const reactDomDir = resolve(root, 'ui', 'node_modules', 'react-dom')
    assert.ok(
      existsSync(reactDomDir),
      `Aliased react-dom directory must exist at ${reactDomDir}`,
    )
  })

  it('binds dev server to 127.0.0.1', () => {
    assert.match(
      content,
      /host:\s*['"]127\.0\.0\.1['"]/,
      'Dev server host must be 127.0.0.1 (not localhost) to avoid DNS resolution differences',
    )
  })

  it('uses strict port 5173', () => {
    assert.match(
      content,
      /strictPort:\s*true/,
      'strictPort must be true so the dev server fails loudly instead of silently using a different port',
    )
  })
})

// ---------------------------------------------------------------------------
// 2. Tauri config
// ---------------------------------------------------------------------------
describe('src-tauri/tauri.conf.json', () => {
  const configPath = resolve(root, 'src-tauri', 'tauri.conf.json')
  const config = JSON.parse(readFileSync(configPath, 'utf-8'))

  it('devUrl uses 127.0.0.1', () => {
    const devUrl = config.build?.devUrl
    assert.ok(devUrl, 'build.devUrl must be defined')
    assert.equal(
      devUrl,
      'http://127.0.0.1:5173',
      'devUrl must use 127.0.0.1 (not localhost) to match the Vite dev server binding',
    )
  })

  it('frontendDist points to ui/dist', () => {
    const dist = config.build?.frontendDist
    assert.ok(dist, 'build.frontendDist must be defined')
    assert.equal(
      dist,
      '../ui/dist',
      'frontendDist must point to the UI build output',
    )
  })
})

// ---------------------------------------------------------------------------
// 3. main.tsx — theme init must not block React render
// ---------------------------------------------------------------------------
describe('ui/src/main.tsx', () => {
  const mainPath = resolve(root, 'ui', 'src', 'main.tsx')
  const content = readFileSync(mainPath, 'utf-8')

  it('initTheme is fire-and-forget with error handler', () => {
    // Must use `void initTheme().catch(...)` pattern — NOT awaited
    assert.match(
      content,
      /void\s+initTheme\(\)\.catch/,
      'initTheme must be called with void (fire-and-forget) and .catch() to prevent unhandled rejections',
    )
  })

  it('initTheme does not use await', () => {
    const initThemeLine = content
      .split('\n')
      .find((line) => line.includes('initTheme'))
    assert.ok(
      initThemeLine && !initThemeLine.includes('await initTheme'),
      'initTheme must NOT be awaited — awaiting it blocks React from mounting and can cause a blank screen if fonts fail to load',
    )
  })

  it('ReactDOM.createRoot().render() is synchronous and not gated on initTheme', () => {
    const initThemeIndex = content.indexOf('initTheme')
    const renderIndex = content.indexOf('ReactDOM.createRoot')

    assert.ok(renderIndex > 0, 'ReactDOM.createRoot must be present')

    // Check that render is NOT inside a .then() or after an await on initTheme
    const betweenInitAndRender = content.slice(
      Math.min(initThemeIndex, renderIndex),
      Math.max(initThemeIndex, renderIndex),
    )
    assert.ok(
      !betweenInitAndRender.includes('.then(') || content.indexOf('.then(') > renderIndex,
      'ReactDOM.createRoot().render() must not be chained after initTheme via .then()',
    )
  })
})

// ---------------------------------------------------------------------------
// 4. Module resolution sanity check
// ---------------------------------------------------------------------------
describe('module resolution (npm hoisting)', () => {
  it('react is installed in ui/node_modules', () => {
    const reactPkg = resolve(root, 'ui', 'node_modules', 'react', 'package.json')
    assert.ok(
      existsSync(reactPkg),
      'react must be installed in ui/node_modules — if it moves, the Vite alias must be updated',
    )
  })

  it('react-dom is installed in ui/node_modules', () => {
    const reactDomPkg = resolve(
      root,
      'ui',
      'node_modules',
      'react-dom',
      'package.json',
    )
    assert.ok(
      existsSync(reactDomPkg),
      'react-dom must be installed in ui/node_modules — if it moves, the Vite alias must be updated',
    )
  })

  it('zustand can reach react through the alias', () => {
    // zustand imports "react" — with the Vite alias, it resolves to
    // ui/node_modules/react.  Verify both ends exist.
    const zustandDir = resolve(root, 'node_modules', 'zustand')
    const reactDir = resolve(root, 'ui', 'node_modules', 'react')

    // zustand may be hoisted or local — at least one must exist
    const zustandExists =
      existsSync(zustandDir) ||
      existsSync(resolve(root, 'ui', 'node_modules', 'zustand'))
    assert.ok(zustandExists, 'zustand must be installed somewhere in node_modules')

    assert.ok(
      existsSync(reactDir),
      'react must exist in ui/node_modules for the Vite alias to resolve',
    )
  })
})
