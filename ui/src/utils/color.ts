function expandHex(hex: string): string {
  if (hex.length === 4) {
    return `#${hex[1]}${hex[1]}${hex[2]}${hex[2]}${hex[3]}${hex[3]}`
  }
  return hex
}

function parseChannel(pair: string): number {
  return Number.parseInt(pair, 16)
}

export function mixHex(a: string, b: string, weightA: number): string {
  const [ar, ag, ab] = [1, 3, 5].map((i) => parseChannel(expandHex(a).slice(i, i + 2)))
  const [br, bg, bb] = [1, 3, 5].map((i) => parseChannel(expandHex(b).slice(i, i + 2)))
  const channel = (x: number, y: number) => Math.round(x * weightA + y * (1 - weightA))
  const toHex = (value: number) => value.toString(16).padStart(2, '0')
  return `#${toHex(channel(ar, br))}${toHex(channel(ag, bg))}${toHex(channel(ab, bb))}`
}
