import { ref } from 'vue'
import { createHead } from '@unhead/vue/client'
import { loadBranding } from './composables/useBranding'
import { loadLocalizations } from './composables/useLocalizations'
import type { Branding, BrandingPalette } from './types/branding'
import { ramp } from './utils/color'

export const bootFailed = ref(false)

/** Single head manager shared between boot-time branding and the Vue app. */
export const head = createHead()

export async function runBoot(): Promise<void> {
  try {
    const [branding] = await Promise.all([loadBranding(), loadLocalizations()])
    applyBootBranding(branding)
    bootFailed.value = false
  } catch {
    bootFailed.value = true
  }
}

function fontFace(family: string, url: string, format: string | null | undefined, style: 'normal' | 'italic') {
  const formatSrc = format ? ` format("${format}")` : ''
  return [
    '@font-face {',
    `  font-family: ${JSON.stringify(family)};`,
    `  src: url(${url})${formatSrc};`,
    '  font-weight: 100 900;',
    `  font-style: ${style};`,
    '  font-display: swap;',
    '}'
  ].join('\n')
}

function fontStackCss(branding: Branding) {
  return `:root {
  --font-sans: ${JSON.stringify(branding.fontNormal.family)}, ui-sans-serif, system-ui, sans-serif;
  --font-mono: ${JSON.stringify(branding.fontMono.family)}, ui-monospace, SFMono-Regular, Menlo, monospace;
}`
}

function injectFonts(branding: Branding) {
  const faces = [fontFace(branding.fontNormal.family, branding.fontNormal.url, branding.fontNormal.format, 'normal')]
  if (branding.fontItalic) {
    faces.push(fontFace(branding.fontNormal.family, branding.fontItalic.url, branding.fontItalic.format, 'italic'))
  }
  faces.push(fontFace(branding.fontMono.family, branding.fontMono.url, branding.fontMono.format, 'normal'))

  const style = document.createElement('style')
  style.setAttribute('data-branding-fonts', '')
  style.textContent = `${faces.join('\n')}\n${fontStackCss(branding)}`
  document.head.appendChild(style)
}

function rampLines(
  name: string,
  seed: string,
  seedShade: string,
  overrides: Record<string, string> = {}
) {
  return ramp(seed, seedShade, overrides)
    .map(({ shade, hex }) => `  --color-${name}-${shade}: ${hex};`)
    .join('\n')
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
  --ui-text-dimmed: var(--color-ink-400);
  --ui-text-muted: var(--color-ink-500);
  --ui-text-toned: var(--color-ink-600);
  --ui-text: var(--color-ink-700);
  --ui-text-highlighted: var(--color-ink-900);
  --ui-text-inverted: #fff;
}`

  const darkSurface = `.dark {
  --ui-bg: var(--color-surface-900);
  --ui-bg-muted: var(--color-surface-800);
  --ui-bg-elevated: var(--color-surface-800);
  --ui-bg-accented: var(--color-surface-700);
  --ui-border: var(--color-surface-800);
  --ui-border-muted: var(--color-surface-700);
  --ui-border-accented: var(--color-surface-700);
  --ui-text-dimmed: var(--color-surface-500);
  --ui-text-muted: var(--color-surface-400);
  --ui-text-toned: var(--color-surface-300);
  --ui-text: var(--color-surface-100);
  --ui-text-highlighted: var(--color-surface-50);
  --ui-text-inverted: var(--color-surface-900);
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

let brandHeadEntry: ReturnType<typeof head.push> | null = null

function applyBootBranding(branding: Branding) {
  const description = branding.tagline || branding.name
  const input = {
    title: branding.name,
    meta: [
      { name: 'description', content: description },
      { property: 'og:title', content: branding.name },
      { property: 'og:description', content: description }
    ],
    link: [{ rel: 'icon', href: branding.faviconUrl }]
  }

  if (brandHeadEntry) {
    brandHeadEntry.patch(input)
  } else {
    brandHeadEntry = head.push(input)
  }

  injectFonts(branding)
  injectThemeColors(branding.colors)
}
