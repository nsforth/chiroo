import { ref } from 'vue'
import { apiFetch } from '../api'
import { brandingSchema, type Branding } from '../types/branding'

const branding = ref<Branding | null>(null)

export async function loadBranding(): Promise<Branding> {
  const data: unknown = await apiFetch('/api/branding')
  const parsed = brandingSchema.parse(data)
  branding.value = parsed
  return parsed
}

export function useBranding() {
  return { branding }
}
