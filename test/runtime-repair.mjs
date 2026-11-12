/**
 * Tests for scripts/repair-openvpn.sh, the in-app "Repair OpenVPN" action that
 * fixes an OpenVPN left broken or outdated by a Homebrew update.
 */

import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const script = fileURLToPath(new URL('../scripts/repair-openvpn.sh', import.meta.url))

const WORKING = "#!/bin/sh\necho 'OpenVPN 2.7.9 aarch64-apple-darwin [SSL (OpenSSL)]'\n"
const BROKEN = "#!/bin/sh\necho 'dyld: Library not loaded: liblz4.1.dylib' >&2\nexit 6\n"

/**
 * Runs the repair with a fake Homebrew. `fixedBy` names the brew subcommand
 * that leaves a working OpenVPN behind ("upgrade", "install", "reinstall").
 */
function repair(t, options = {}) {
  const dir = mkdtempSync(resolve(tmpdir(), 'openwrap-repair-'))
  t.after(() => rmSync(dir, { recursive: true, force: true }))
  const openvpn = resolve(dir, 'openvpn')
  if (options.installed !== false) writeFileSync(openvpn, BROKEN, { mode: 0o755 })
  writeFileSync(resolve(dir, 'working'), WORKING)
  writeFileSync(
    resolve(dir, 'brew'),
    `#!/bin/bash
echo "brew $*" >> "$FIXTURE/calls"
case "$1" in
  update) exit ${options.updateExit ?? 0} ;;
  list) [[ -f "$FIXTURE/openvpn" ]]; exit $? ;;
  ${options.fixedBy ?? 'upgrade'}) cp "$FIXTURE/working" "$FIXTURE/openvpn"; chmod +x "$FIXTURE/openvpn" ;;
esac
exit ${options.brewExit ?? 0}
`,
    { mode: 0o755 },
  )
  const result = spawnSync(
    '/bin/bash',
    [
      '-c',
      `
source "$REPAIR_SCRIPT"
id() { echo "${options.root ? 0 : 501}"; }
find_openvpn() { [[ -x "$FIXTURE/openvpn" ]] && echo "$FIXTURE/openvpn"; }
find_brew() { ${options.noBrew ? 'return 1' : 'echo "$FIXTURE/brew"'}; }
repair_openvpn
`,
    ],
    { encoding: 'utf-8', env: { ...process.env, FIXTURE: dir, REPAIR_SCRIPT: script } },
  )
  const callsPath = resolve(dir, 'calls')
  const calls = existsSync(callsPath) ? readFileSync(callsPath, 'utf-8').trim().split('\n') : []
  return { result, calls }
}

test('a broken OpenVPN is fixed by a Homebrew upgrade', (t) => {
  const { result, calls } = repair(t)
  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(calls, ['brew update', 'brew list --formula openvpn', 'brew upgrade openvpn'])
  assert.match(result.stdout, /OpenVPN is ready: .*openvpn \(2\.7\.9\)/)
})

test('OpenVPN is reinstalled when upgrading does not make it start', (t) => {
  const { result, calls } = repair(t, { fixedBy: 'reinstall' })
  assert.equal(result.status, 0, result.stderr)
  assert.deepEqual(calls.slice(-2), ['brew upgrade openvpn', 'brew reinstall openvpn'])
  assert.match(result.stdout, /OpenVPN is ready/)
})

test('OpenVPN is installed when Homebrew does not have it', (t) => {
  const { result, calls } = repair(t, { installed: false, fixedBy: 'install' })
  assert.equal(result.status, 0, result.stderr)
  assert.ok(calls.includes('brew install openvpn'))
  assert.ok(!calls.includes('brew upgrade openvpn'))
})

test('a failed package list refresh does not stop the repair', (t) => {
  const { result, calls } = repair(t, { updateExit: 1 })
  assert.equal(result.status, 0, result.stderr)
  assert.ok(calls.includes('brew upgrade openvpn'))
})

test('the repair fails when OpenVPN still cannot start', (t) => {
  const { result, calls } = repair(t, { fixedBy: 'nothing' })
  assert.notEqual(result.status, 0)
  assert.ok(calls.includes('brew reinstall openvpn'))
  assert.match(result.stderr, /still does not start/)
  assert.doesNotMatch(result.stdout, /OpenVPN is ready/)
})

test('a failing Homebrew upgrade is reported', (t) => {
  const { result } = repair(t, { fixedBy: 'nothing', brewExit: 1 })
  assert.notEqual(result.status, 0)
  assert.doesNotMatch(result.stdout, /OpenVPN is ready/)
})

test('the repair refuses to run as root or without Homebrew', (t) => {
  const asRoot = repair(t, { root: true })
  assert.notEqual(asRoot.result.status, 0)
  assert.deepEqual(asRoot.calls, [])

  const noBrew = repair(t, { noBrew: true })
  assert.notEqual(noBrew.result.status, 0)
  assert.match(noBrew.result.stderr, /Homebrew is not installed/)
})
