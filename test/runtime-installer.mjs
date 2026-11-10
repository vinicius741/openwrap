import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import { test as baseTest } from 'node:test'
import { fileURLToPath } from 'node:url'

// The installer is a macOS bash script; there is no /bin/bash on Windows.
const test = process.platform === 'win32' ? baseTest.skip : baseTest

const installer = fileURLToPath(new URL('../scripts/share/install-openvpn.sh', import.meta.url))

function fixture(t, options = {}) {
  const dir = mkdtempSync(resolve(tmpdir(), 'openwrap-installer-'))
  t.after(() => rmSync(dir, { recursive: true, force: true }))
  const brew = `#!/bin/bash
set -eu
echo "brew $*" >> "$FIXTURE/calls"
exit_code="${options.brewExit ?? 0}"
if [[ "$exit_code" != 0 ]]; then exit "$exit_code"; fi
${options.noBinary ? ':' : 'touch "$FIXTURE/openvpn"'}
`
  writeFileSync(resolve(dir, 'brew-template'), brew, { mode: 0o755 })
  if (options.hasBrew) {
    writeFileSync(resolve(dir, 'brew'), brew, { mode: 0o755 })
  }
  if (options.hasOpenvpn) writeFileSync(resolve(dir, 'openvpn'), '')
  const result = spawnSync('/bin/bash', ['-c', `
source "$INSTALLER_SCRIPT"
id() { echo "${options.root ? 0 : 501}"; }
find_openvpn() { [[ -f "$FIXTURE/openvpn" ]] && echo "$FIXTURE/openvpn"; }
find_brew() { [[ -x "$FIXTURE/brew" ]] && echo "$FIXTURE/brew"; }
sudo() { echo "sudo $*" >> "$FIXTURE/calls"; return ${options.sudoExit ?? 0}; }
curl() {
  echo 'curl' >> "$FIXTURE/calls"
  ${options.curlExit ? `return ${options.curlExit}` : ''}
  local destination=''
  while [[ $# -gt 0 ]]; do
    if [[ "$1" == '--output' ]]; then destination="$2"; shift; fi
    shift
  done
  cat > "$destination" <<'HOMEBREW'
echo 'homebrew installer' >> "$FIXTURE/calls"
${options.homebrewExit ? `exit ${options.homebrewExit}` : 'cp "$FIXTURE/brew-template" "$FIXTURE/brew"'}
HOMEBREW
}
install_openvpn
`], {
    env: { ...process.env, INSTALLER_SCRIPT: installer, FIXTURE: dir },
    encoding: 'utf8',
    timeout: 10_000,
  })
  assert.ifError(result.error)
  const calls = existsSync(resolve(dir, 'calls')) ? readFileSync(resolve(dir, 'calls'), 'utf8') : ''
  return { ...result, calls, dir }
}

test('existing OpenVPN skips Homebrew and all installation commands', (t) => {
  const result = fixture(t, { hasOpenvpn: true })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.calls, '')
  assert.match(result.stdout, /OpenVPN is ready/)
})

test('existing Homebrew installs only OpenVPN', (t) => {
  const result = fixture(t, { hasBrew: true })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.calls, 'brew install openvpn\n')
})

test('a clean Mac installs Homebrew before OpenVPN', (t) => {
  const result = fixture(t)
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.calls, 'sudo -v\ncurl\nhomebrew installer\nbrew install openvpn\n')
  assert.match(result.stdout, /OpenVPN is ready/)
})

test('cancelled administrator authentication stops before downloading', (t) => {
  const result = fixture(t, { sudoExit: 1 })
  assert.notEqual(result.status, 0)
  assert.equal(result.calls, 'sudo -v\n')
  assert.doesNotMatch(result.stdout, /OpenVPN is ready/)
})

test('a failed download never executes the Homebrew installer', (t) => {
  const result = fixture(t, { curlExit: 22 })
  assert.equal(result.status, 22)
  assert.equal(result.calls, 'sudo -v\ncurl\n')
  assert.doesNotMatch(result.stdout, /OpenVPN is ready/)
})

test('failed Homebrew installation stops before installing OpenVPN', (t) => {
  const result = fixture(t, { homebrewExit: 7 })
  assert.equal(result.status, 7)
  assert.doesNotMatch(result.calls, /brew install openvpn/)
})

test('OpenVPN installation failures are reported', (t) => {
  const result = fixture(t, { hasBrew: true, brewExit: 9 })
  assert.equal(result.status, 9)
  assert.doesNotMatch(result.stdout, /OpenVPN is ready/)
})

test('a successful brew exit without an OpenVPN executable is a failure', (t) => {
  const result = fixture(t, { hasBrew: true, noBinary: true })
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /did not produce an executable/)
})

test('setup refuses to install Homebrew or OpenVPN as root', (t) => {
  const result = fixture(t, { root: true })
  assert.notEqual(result.status, 0)
  assert.equal(result.calls, '')
  assert.match(result.stderr, /without sudo/)
})

test('an executable custom OpenVPN path is reused without other dependencies', (t) => {
  const dir = mkdtempSync(resolve(tmpdir(), 'openwrap-custom-'))
  t.after(() => rmSync(dir, { recursive: true, force: true }))
  const binary = resolve(dir, "My 'VPN' $(no-command)")
  writeFileSync(binary, '#!/bin/sh\nexit 0\n')
  chmodSync(binary, 0o755)
  const result = spawnSync('/bin/bash', [installer, binary], { encoding: 'utf8', timeout: 10_000 })
  assert.equal(result.status, 0, result.stderr)
  assert.match(result.stdout, /OpenVPN is ready/)
  assert.ok(result.stdout.includes(binary))
})
