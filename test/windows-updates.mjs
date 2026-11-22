/**
 * Windows in-app updates.
 *
 * Guards the pieces that must stay in sync for a Windows install to update
 * itself: the manifest entry the release adds, the installer hook that
 * relaunches the elevated app, and the Rust updater settings.
 */

import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { resolve, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

import { addWindowsUpdate, WINDOWS_PLATFORM_KEYS } from '../scripts/add-windows-update.mjs'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const read = (...parts) => readFileSync(resolve(root, ...parts), 'utf-8')

describe('scripts/add-windows-update.mjs', () => {
  const mac = { signature: 'mac-sig', url: 'https://example.test/OpenWrap.app.tar.gz' }
  const manifest = {
    version: '1.2.3',
    notes: 'n',
    pub_date: '2026-01-01T00:00:00Z',
    platforms: { 'darwin-aarch64': mac },
  }
  const url = 'https://github.com/o/r/releases/download/v1.2.3/OpenWrap-Windows-Setup.exe'

  it('adds both Windows keys and keeps the macOS entry', () => {
    const updated = addWindowsUpdate(manifest, 'win-sig\n', url)
    assert.deepEqual(updated.platforms['darwin-aarch64'], mac)
    for (const key of WINDOWS_PLATFORM_KEYS) {
      assert.deepEqual(updated.platforms[key], { signature: 'win-sig', url })
    }
    assert.equal(updated.version, '1.2.3')
    // Input is not mutated.
    assert.deepEqual(Object.keys(manifest.platforms), ['darwin-aarch64'])
  })

  it('rejects an empty signature, a non-https URL and a manifest without platforms', () => {
    assert.throws(() => addWindowsUpdate(manifest, '  \n', url), /signature/)
    assert.throws(() => addWindowsUpdate(manifest, 'sig', 'http://x/y.exe'), /https/)
    assert.throws(() => addWindowsUpdate({ version: '1' }, 'sig', url), /platforms/)
  })
})

describe('Windows update install path', () => {
  it('relaunches the elevated app from the installer hook in update mode', () => {
    const hooks = read('src-tauri', 'windows', 'hooks.nsh')
    const post = hooks.slice(hooks.indexOf('!macro NSIS_HOOK_POSTINSTALL'))
    assert.match(post, /StrCmp \$UpdateMode 1/)
    assert.match(post, /ExecShell "" "\$INSTDIR\\\$\{MAINBINARYNAME\}\.exe"/)
    // The relaunch must come before (and skip) the OpenVPN prompt.
    assert.ok(post.indexOf('ExecShell') < post.indexOf('install-openvpn.ps1'))
  })

  it('does not ask the installer for the RunAsUser relaunch and shuts down before exit', () => {
    const updates = read('src-tauri', 'src', 'commands', 'updates.rs')
    assert.match(updates, /\.restart_after_install\(false\)/)
    assert.match(updates, /\.on_before_exit\(/)
    assert.match(updates, /state\.shutdown\(\)/)
  })

  it('checks for updates on every platform', () => {
    const hook = read('ui', 'src', 'features', 'updates', 'useUpdateEvents.ts')
    assert.doesNotMatch(hook, /isWindows/)
    const settings = read('ui', 'src', 'features', 'settings', 'SettingsView.tsx')
    assert.doesNotMatch(settings, /Automatic updates are not available on Windows/)
  })

  it('publishes a signed Windows installer in latest.json', () => {
    const workflow = read('.github', 'workflows', 'release.yml')
    assert.match(workflow, /signer sign/)
    assert.match(workflow, /OpenWrap-Windows-Setup\.exe\.sig/)
    assert.match(workflow, /add-windows-update\.mjs/)
    assert.match(workflow, /--draft/)
  })
})
