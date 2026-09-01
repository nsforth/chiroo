import type { UIMessage } from 'ai'
import { isReasoningUIPart, isTextUIPart, isToolUIPart, getToolName } from 'ai'
import { isPartStreaming, isToolStreaming } from '@nuxt/ui/utils/ai'
import { getMergedParts } from './ai'
import { getSearchQuery, getSearchQueries, getExtractedDocument } from './tool'
import type { ToolPart } from './tool'

export type StreamingStatus =
  | { type: 'sending-request' }
  | { type: 'thinking' }
  | { type: 'answering' }
  | { type: 'searching'; query: string }
  | { type: 'reading-document' }
  | { type: 'read-document'; title: string }
  | { type: 'running-tool'; tool: string }
  | { type: 'generating-chart' }
  | { type: 'loading-weather' }

export function isStreamingStatusVisible(status: string): boolean {
  return status === 'submitted' || status === 'streaming'
}

export function resolveStreamingStatus(status: string, messages: UIMessage[]): StreamingStatus | null {
  if (status === 'submitted') return { type: 'sending-request' }
  if (status !== 'streaming') return null

  const message = getLastAssistantMessage(messages)
  if (!message) return { type: 'thinking' }

  return resolveMessageStatus(message)
}

function getLastAssistantMessage(messages: UIMessage[]): UIMessage | null {
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].role === 'assistant') return messages[i]
  }
  return null
}

function resolveMessageStatus(message: UIMessage): StreamingStatus {
  const parts = getMergedParts(message.parts).filter(part => part.type !== 'step-start')

  for (let i = parts.length - 1; i >= 0; i--) {
    const status = resolvePartStatus(parts[i], i === parts.length - 1)
    if (status) return status
  }

  return { type: 'thinking' }
}

function resolvePartStatus(part: UIMessage['parts'][number], isLast: boolean): StreamingStatus | null {
  if (isReasoningUIPart(part)) {
    return isPartStreaming(part) ? { type: 'thinking' } : null
  }

  if (isToolUIPart(part)) {
    if (isToolStreaming(part)) return resolveToolStreamingStatus(part)
    if (isLast && getToolName(part) === 'extract_document') {
      return { type: 'read-document', title: getExtractedDocument(part)?.title ?? '' }
    }
    return null
  }

  if (isTextUIPart(part)) {
    return isPartStreaming(part) ? { type: 'answering' } : null
  }

  return null
}

function resolveToolStreamingStatus(part: ToolPart): StreamingStatus {
  switch (getToolName(part)) {
    case 'search_knowledge_base':
      return { type: 'searching', query: getSearchQueries(part).join(', ') }
    case 'web_search':
    case 'google_search':
      return { type: 'searching', query: getSearchQuery(part) ?? '' }
    case 'extract_document':
      return { type: 'reading-document' }
    case 'chart':
      return { type: 'generating-chart' }
    case 'weather':
      return { type: 'loading-weather' }
    default:
      return { type: 'running-tool', tool: getToolName(part) }
  }
}
