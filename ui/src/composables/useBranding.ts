import { ref } from 'vue'
import { apiFetch } from '../api'
import type { Branding, BrandingPalette, FontDescriptor } from '../types/branding'

const branding = ref<Branding | null>(null)

function isFontDescriptor(value: unknown): value is FontDescriptor {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Record<string, unknown>
  return typeof candidate.family === 'string'
    && typeof candidate.url === 'string'
    && (candidate.format === undefined || candidate.format === null || typeof candidate.format === 'string')
}

function isHexColor(value: unknown): value is string {
  return typeof value === 'string' && /^#[0-9a-fA-F]{3}([0-9a-fA-F]{3})?$/.test(value)
}

function isBrandingPalette(value: unknown): value is BrandingPalette {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Record<string, unknown>
  return isHexColor(candidate.primary)
    && isHexColor(candidate.neutral)
    && isHexColor(candidate.primaryDark)
    && isHexColor(candidate.neutralDark)
}

function isBranding(value: unknown): value is Branding {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Record<string, unknown>
  return typeof candidate.name === 'string'
    && typeof candidate.assistantName === 'string'
    && typeof candidate.logoUrl === 'string'
    && typeof candidate.faviconUrl === 'string'
    && typeof candidate.assistantUrl === 'string'
    && (candidate.colors === undefined || candidate.colors === null || isBrandingPalette(candidate.colors))
    && (candidate.tagline === undefined || typeof candidate.tagline === 'string')
    && (candidate.fontSans === undefined || candidate.fontSans === null || isFontDescriptor(candidate.fontSans))
    && (candidate.fontMono === undefined || candidate.fontMono === null || isFontDescriptor(candidate.fontMono))
}

export async function loadBranding(): Promise<Branding> {
  const data: unknown = await apiFetch('/api/branding')
  if (!isBranding(data)) {
    throw new Error('Invalid branding payload')
  }
  branding.value = data
  return data
}

export function useBranding() {
  return { branding }
}
