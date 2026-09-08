import { formatHex, interpolate } from 'culori'

const RAMP_SHADES = ['50', '100', '200', '300', '400', '500', '600', '700', '800', '900', '950'] as const

const LIGHT_WEIGHT: Record<string, number> = {
  '50': 0.05, '100': 0.09, '200': 0.16, '300': 0.26, '400': 0.38, '500': 0.50,
  '600': 0.62, '700': 0.76, '800': 0.90
}
const DARK_WEIGHT: Record<string, number> = {
  '600': 0.92, '700': 0.85, '800': 0.75, '900': 0.62, '950': 0.50
}

function mix(a: string, b: string, t: number): string {
  return formatHex(interpolate([a, b], 'oklab')(t))
}

function lighten(seed: string, weight: number): string {
  return mix('#ffffff', seed, weight)
}

function darken(seed: string, weight: number): string {
  return mix('#000000', seed, weight)
}

export interface RampShade {
  shade: string
  hex: string
}

/** Builds an 11-stop ramp from a seed, lightening shades below `seedShade` and darkening above it. */
export function ramp(seed: string, seedShade: string, overrides: Record<string, string> = {}): RampShade[] {
  return RAMP_SHADES.map((shade) => {
    if (overrides[shade]) return { shade, hex: overrides[shade] }
    if (shade === seedShade) return { shade, hex: seed }
    const hex = Number(shade) < Number(seedShade)
      ? lighten(seed, LIGHT_WEIGHT[shade])
      : darken(seed, DARK_WEIGHT[shade])
    return { shade, hex }
  })
}

/** Mixes two hex colors in the oklab space (as CSS `color-mix` does). */
export function mixHex(a: string, b: string, weightA: number): string {
  return mix(b, a, weightA)
}
