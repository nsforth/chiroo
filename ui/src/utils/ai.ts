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

export function getMessageText(message: UIMessage): string {
  return message.parts
    .filter(part => part.type === 'text')
    .map(part => (part as { text: string }).text)
    .join('\n\n')
}
