export interface ModelOption {
  label: string
  value: string
  icon: string
  iconUrl?: string
  description?: string
}

const ICONS: Record<string, string> = {
  sparkles: 'i-lucide-sparkles',
  compass: 'i-lucide-compass'
}

export function iconByName(name: string): string {
  return ICONS[name] ?? 'i-lucide-bot'
}
