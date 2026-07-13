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

  it('builds and bundles the privileged helper as a legacy resource', () => {
    assert.equal(
      config.build?.beforeBuildCommand,
      'cargo build -p openwrap-helper --release && npm run build --workspace ui',
    )
    assert.equal(
      config.bundle?.resources?.['../target/release/openwrap-helper'],
      'openwrap-helper-bundled',
    )
  })

  it('configures macOS Packet Tunnel entitlements and Info.plist merge', () => {
    assert.equal(config.bundle?.macOS?.entitlements, 'OpenWrap.entitlements')
    assert.equal(config.bundle?.macOS?.infoPlist, 'Info.plist')
    assert.ok(
      existsSync(resolve(root, 'src-tauri', 'OpenWrap.entitlements')),
      'host entitlements file must exist',
    )
    assert.ok(
      existsSync(resolve(root, 'src-tauri', 'Info.plist')),
      'host Info.plist merge file must exist',
    )
  })
})

// ---------------------------------------------------------------------------
// Hobby constraint: default helper path must stay free (no paid Apple NE).
// Packet Tunnel may exist as opt-in code but must never become the default
// without an explicit maintainer decision to pay for Developer Program NE.
// ---------------------------------------------------------------------------
describe('hobby backend constraint (helper default)', () => {
  const packageJson = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf-8'))
  const readme = readFileSync(resolve(root, 'README.md'), 'utf-8')
  const agents = readFileSync(resolve(root, 'AGENTS.md'), 'utf-8')
  const signedScript = resolve(root, 'scripts', 'build-signed-app.sh')
  const backendFactory = readFileSync(
    resolve(root, 'src-tauri', 'src', 'app_state', 'backend_factory.rs'),
    'utf-8',
  )

  it('documents the intentional hobby / no paid Apple fee constraint', () => {
    assert.match(agents, /Hobby constraint/)
    assert.match(agents, /will not pay|will \*\*not\*\* pay/i)
    assert.match(backendFactory, /hobby project/i)
    // Module docs may use markdown bold: will **not** pay
    assert.match(backendFactory, /will \*\*not\*\* pay|will not pay/i)
    assert.match(readme, /Hobby project|hobby/i)
  })

  it('keeps Packet Tunnel signed tooling available but opt-in', () => {
    assert.equal(packageJson.scripts?.['tauri:build:signed'], './scripts/build-signed-app.sh')
    assert.ok(existsSync(signedScript), 'scripts/build-signed-app.sh must exist')
    const content = readFileSync(signedScript, 'utf-8')
    assert.match(content, /build-network-extension\.sh/)
    assert.match(content, /SystemExtensions/)
    assert.match(content, /app\.openwrap\.desktop\.PacketTunnel\.systemextension/)
  })

  it('defaults to the free helper backend; Packet Tunnel is opt-in only', () => {
    assert.match(backendFactory, /HelperOpenVpnBackend/)
    assert.match(backendFactory, /NetworkExtensionBackend/)
    assert.match(backendFactory, /prefer_packet_tunnel_backend|OPENWRAP_VPN_BACKEND/)
    // Default must not select Packet Tunnel without an explicit env opt-in.
    assert.match(backendFactory, /Err\(_\) => false/)
  })

  it('documents both the helper hobby path and optional signed Packet Tunnel builds', () => {
    assert.match(readme, /helper|Helper/)
    assert.match(readme, /Packet Tunnel|tauri:build:signed/)
  })
})

describe('privileged helper installation', () => {
  const readme = readFileSync(resolve(root, 'README.md'), 'utf-8')
  const installScript = resolve(root, 'scripts', 'install-helper.sh')

  it('documents the explicit sudo installation command', () => {
    assert.match(readme, /sudo \.\/scripts\/install-helper\.sh/)
  })

  it('ships a path-checked helper installer script', () => {
    assert.ok(existsSync(installScript), 'scripts/install-helper.sh must exist')
    const content = readFileSync(installScript, 'utf-8')
    assert.match(content, /\/Library\/PrivilegedHelperTools/)
    assert.match(content, /-m 4755/)
    assert.match(content, /openwrap-helper/)
    assert.match(content, /target\//)
    assert.match(content, /Refusing to install/)
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
