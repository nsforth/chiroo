import { formatHex, interpolate } from 'culori'

/** Mixes two hex colors in the oklab space (as CSS `color-mix` does). */
export function mixHex(a: string, b: string, weightA: number): string {
  const mix = interpolate([b, a], 'oklab')
  return formatHex(mix(weightA))
}
