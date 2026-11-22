#!/usr/bin/env node
// Adds the Windows NSIS installer to the updater manifest (latest.json) that
// scripts/package-release.sh wrote for macOS.
//
//   node scripts/add-windows-update.mjs <latest.json> <installer.exe.sig> <installer url>
//
// tauri-plugin-updater looks up `windows-x86_64-nsis` first and falls back to
// `windows-x86_64`, so both keys point at the same signed installer.

import { readFileSync, writeFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

export const WINDOWS_PLATFORM_KEYS = ['windows-x86_64', 'windows-x86_64-nsis']

export function addWindowsUpdate(manifest, signature, url) {
  if (!manifest || typeof manifest !== 'object' || !manifest.platforms) {
    throw new Error('latest.json has no platforms object')
  }
  const sig = String(signature).trim()
  if (!sig) throw new Error('Windows installer signature is empty')
  if (!/^https:\/\//.test(url)) throw new Error(`Windows installer URL must be https: ${url}`)

  const platforms = { ...manifest.platforms }
  for (const key of WINDOWS_PLATFORM_KEYS) {
    platforms[key] = { signature: sig, url }
  }
  return { ...manifest, platforms }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  const [manifestPath, sigPath, url] = process.argv.slice(2)
  if (!manifestPath || !sigPath || !url) {
    console.error('usage: add-windows-update.mjs <latest.json> <installer.exe.sig> <installer url>')
    process.exit(2)
  }
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf-8'))
  const updated = addWindowsUpdate(manifest, readFileSync(sigPath, 'utf-8'), url)
  writeFileSync(manifestPath, JSON.stringify(updated, null, 2) + '\n')
  console.log(`Added ${WINDOWS_PLATFORM_KEYS.join(', ')} to ${manifestPath}`)
}
