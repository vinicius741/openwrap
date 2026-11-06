import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { test } from 'node:test'

const requireUi = createRequire(new URL('../ui/package.json', import.meta.url))
const { loadConfigFromFile } = await import(pathToFileURL(requireUi.resolve('vite')).href)
const loaded = await loadConfigFromFile(
  { command: 'build', mode: 'production' },
  fileURLToPath(new URL('../ui/vite.config.ts', import.meta.url)),
)
const plugin = loaded.config.plugins.flat().find((plugin) => plugin?.name === 'fontsource-woff2-only')
const id = '/node_modules/@fontsource/inter/400.css'

test('font transform keeps every WOFF2 face and removes only legacy fallbacks', () => {
  assert.ok(plugin, 'The WOFF2 transform must be registered in the real Vite config')
  assert.equal(plugin.enforce, 'pre')
  const source = `@font-face {
  font-weight: 400;
  src: url(./latin.woff2) format('woff2'), url(./latin.woff) format('woff');
  unicode-range: U+0000-00FF;
}
@font-face {
  font-weight: 700;
  src: url(./cyrillic.woff2) format("woff2"), url(./cyrillic.woff) format("woff");
  unicode-range: U+0400-045F;
}`
  const expected = source
    .replace(", url(./latin.woff) format('woff')", '')
    .replace(', url(./cyrillic.woff) format("woff")', '')
  assert.equal(plugin.transform(source, id), expected)
})

test('font transform leaves WOFF2-only CSS intact', () => {
  const source = "@font-face { src: url(./font.woff2) format('woff2'); }"
  assert.equal(plugin.transform(source, id), source)
})

test('font transform is scoped to Fontsource CSS files', () => {
  const source = "url(./font.woff2) format('woff2'), url(./font.woff) format('woff')"
  for (const otherId of ['/src/custom.css', '/node_modules/@fontsource/inter/index.js', `${id}?v=123`]) {
    assert.equal(plugin.transform(source, otherId), undefined)
  }
})
