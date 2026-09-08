import type { UIMessage } from 'ai'
import { isTextUIPart } from 'ai'

export function getMergedParts(parts: UIMessage['parts']): UIMessage['parts'] {
  const result: UIMessage['parts'] = []
  for (const part of parts) {
    const prev = result[result.length - 1]
    if (part.type === 'source-url') {
      continue
    }
    if (isTextUIPart(part) && prev && isTextUIPart(prev)) {
      result[result.length - 1] = { type: 'text', text: prev.text + part.text }
    } else {
      result.push(part)
    }
  }
  return result
}

/**
 * Parts to actually render for a message. Drops structural markers
 * (`step-start`, `source-url`), reads `data-round` markers to track which
 * rounds are hidden, and — for non-admins (`showHidden: false`) — filters out
 * parts belonging to hidden rounds. Adjacent text parts are merged, matching
 * `getMergedParts`.
 */
export function getRenderableParts(message: UIMessage, showHidden: boolean): UIMessage['parts'] {
  const result: UIMessage['parts'] = []
  let hidden = false
  for (const part of message.parts) {
    if (part.type === 'source-url' || part.type === 'step-start') {
      continue
    }
    if (part.type === 'data-round') {
      const data = (part as unknown as { data?: { hidden?: unknown } }).data
      hidden = data?.hidden === true
      continue
    }
    if (hidden && !showHidden) {
      continue
    }
    const prev = result[result.length - 1]
    if (isTextUIPart(part) && prev && isTextUIPart(prev)) {
      result[result.length - 1] = { type: 'text', text: prev.text + part.text }
    } else {
      result.push(part)
    }
  }
  return result
}

export interface MessageSource {
  sourceId: string
  url: string
  title?: string
  score?: number
}

export function getMessageSources(message: UIMessage): MessageSource[] {
  const seen = new Set<string>()
  const sources: MessageSource[] = []
  for (const part of message.parts) {
    if (part.type !== 'source-url') continue
    if (seen.has(part.sourceId)) continue
    seen.add(part.sourceId)
    const score = part.providerMetadata?.chiroo?.score
    sources.push({
      sourceId: part.sourceId,
      url: part.url,
      title: part.title,
      score: typeof score === 'number' ? score : undefined
    })
  }
  return sources.sort((a, b) => (b.score ?? -Infinity) - (a.score ?? -Infinity))
}

export function getMessageText(message: UIMessage, showHidden: boolean): string {
  return getRenderableParts(message, showHidden)
    .filter(part => part.type === 'text')
    .map(part => (part as { text: string }).text)
    .join('\n\n')
}
