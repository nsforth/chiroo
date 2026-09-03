import './assets/css/main.css'

import { createApp } from 'vue'
import type { RouteRecordRaw } from 'vue-router'
import { createRouter, createWebHistory } from 'vue-router'
import { routes, handleHotUpdate } from 'vue-router/auto-routes'
import { setupLayouts } from 'virtual:generated-layouts'
import { createHead } from '@unhead/vue/client'
import ui from '@nuxt/ui/vue-plugin'

import App from './App.vue'
import BootError from './components/BootError.vue'
import { i18n } from './plugins/i18n'
import { loadBranding } from './composables/useBranding'
import { loadLocalizations } from './composables/useLocalizations'
import type { Branding, BrandingPalette, FontDescriptor } from './types/branding'

function setMeta(attribute: 'name' | 'property', key: string, content: string) {
  const selector = `meta[${attribute}="${key}"]`
  let meta = document.head.querySelector<HTMLMetaElement>(selector)
  if (!meta) {
    meta = document.createElement('meta')
    meta.setAttribute(attribute, key)
    document.head.appendChild(meta)
  }
  meta.content = content
}

function setFavicon(href: string) {
  let icon = document.head.querySelector<HTMLLinkElement>('link[rel="icon"]')
  if (!icon) {
    icon = document.createElement('link')
    icon.rel = 'icon'
    document.head.appendChild(icon)
  }
  icon.href = href
}

function cssString(value: string) {
  return `"${value.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`
}

function fontFaceRule(font: FontDescriptor) {
  const format = font.format ? ` format("${font.format}")` : ''
  return [
    '@font-face {',
    `  font-family: ${cssString(font.family)};`,
    `  src: url(${font.url})${format};`,
    '  font-weight: 100 900;',
    '  font-style: normal;',
    '  font-display: swap;',
    '}'
  ].join('\n')
}

function fontStackCss(branding: Branding) {
  const declarations: string[] = []
  if (branding.fontSans) {
    declarations.push(`--font-sans: ${cssString(branding.fontSans.family)}, ui-sans-serif, system-ui, sans-serif;`)
  }
  if (branding.fontMono) {
    declarations.push(`--font-mono: ${cssString(branding.fontMono.family)}, ui-monospace, SFMono-Regular, Menlo, monospace;`)
  }
  return declarations.length ? `:root {\n  ${declarations.join('\n  ')}\n}` : ''
}

function injectFonts(branding: Branding) {
  const fontFaces = [branding.fontSans, branding.fontMono]
    .filter((font): font is FontDescriptor => Boolean(font))
    .map(fontFaceRule)
  const css = [...fontFaces, fontStackCss(branding)].filter(Boolean).join('\n')
  if (!css) return

  const style = document.createElement('style')
  style.setAttribute('data-branding-fonts', '')
  style.textContent = css
  document.head.appendChild(style)
}

const RAMP_SHADES = ['50', '100', '200', '300', '400', '500', '600', '700', '800', '900', '950'] as const
const LIGHT_WEIGHT: Record<string, number> = {
  '50': 0.05, '100': 0.09, '200': 0.16, '300': 0.26, '400': 0.38, '500': 0.50,
  '600': 0.62, '700': 0.76, '800': 0.90
}
const DARK_WEIGHT: Record<string, number> = {
  '600': 0.92, '700': 0.85, '800': 0.75, '900': 0.62, '950': 0.50
}

function mixLight(seed: string, weight: number) {
  return `color-mix(in oklab, ${seed} ${Math.round(weight * 100)}%, #fff)`
}

function mixDark(seed: string, weight: number) {
  return `color-mix(in oklab, ${seed} ${Math.round(weight * 100)}%, #000)`
}

function rampLines(
  name: string,
  seed: string,
  seedShade: string,
  overrides: Record<string, string> = {}
) {
  return RAMP_SHADES.map((shade) => {
    if (overrides[shade]) return `  --color-${name}-${shade}: ${overrides[shade]};`
    if (shade === seedShade) return `  --color-${name}-${shade}: ${seed};`
    const value = Number(shade) < Number(seedShade)
      ? mixLight(seed, LIGHT_WEIGHT[shade])
      : mixDark(seed, DARK_WEIGHT[shade])
    return `  --color-${name}-${shade}: ${value};`
  }).join('\n')
}

function themeColorsCss(palette: BrandingPalette) {
  const ramps = [
    rampLines('brand', palette.primary, '500', { '400': palette.primaryDark }),
    rampLines('ink', palette.neutral, '900'),
    rampLines('surface', palette.neutralDark, '900')
  ].join('\n')

  const lightSurface = `:root {
${ramps}
  --ui-bg: var(--color-ink-50);
  --ui-bg-muted: var(--color-ink-50);
  --ui-bg-elevated: var(--color-ink-100);
  --ui-bg-accented: var(--color-ink-200);
  --ui-border: var(--color-ink-100);
  --ui-border-muted: var(--color-ink-100);
  --ui-border-accented: var(--color-ink-200);
  --ui-text: var(--color-ink-900);
  --ui-text-highlighted: var(--color-ink-900);
}`

  const darkSurface = `.dark {
  --ui-bg: var(--color-surface-900);
  --ui-bg-muted: var(--color-surface-800);
  --ui-bg-elevated: var(--color-surface-800);
  --ui-bg-accented: var(--color-surface-700);
  --ui-border: var(--color-surface-800);
  --ui-border-muted: var(--color-surface-700);
  --ui-border-accented: var(--color-surface-700);
}`

  const backgrounds = [
    'html { background-color: var(--color-ink-50); }',
    'html.dark { background-color: var(--color-surface-900); }',
    'body { background-color: var(--color-ink-50); }',
    '.dark body { background-color: var(--color-surface-900); }'
  ].join('\n')

  return `${lightSurface}\n\n${darkSurface}\n\n${backgrounds}`
}

function injectThemeColors(palette: BrandingPalette | null) {
  if (!palette) return
  const style = document.createElement('style')
  style.setAttribute('data-branding-colors', '')
  style.textContent = themeColorsCss(palette)
  document.head.appendChild(style)
}

function applyBootBranding(branding: Branding) {
  document.title = branding.name
  setFavicon(branding.faviconUrl)
  const description = branding.tagline || branding.name
  setMeta('name', 'description', description)
  setMeta('property', 'og:title', branding.name)
  setMeta('property', 'og:description', description)
  injectFonts(branding)
  injectThemeColors(branding.colors)
}

function mountMainApp() {
  const head = createHead()
  const router = createRouter({
    routes: setupLayouts(routes as RouteRecordRaw[]),
    history: createWebHistory()
  })

  const app = createApp(App)

  app.use(head)
  app.use(router)
  app.use(ui)
  app.use(i18n)

  app.mount('#app')

  // This will update routes at runtime without reloading the page
  if (import.meta.hot) {
    handleHotUpdate(router)
  }
}

function mountBootError() {
  const app = createApp(BootError)
  app.use(i18n)
  app.mount('#app')
}

async function boot() {
  try {
    const [branding] = await Promise.all([loadBranding(), loadLocalizations()])
    applyBootBranding(branding)
  } catch {
    mountBootError()
    return
  }
  mountMainApp()
}

void boot()
