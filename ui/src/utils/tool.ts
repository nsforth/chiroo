import type { getToolName } from 'ai'

export interface Source {
  url: string
  title?: string
}

interface GoogleGroundingChunk {
  web?: { uri?: string, title?: string }
}

interface SearchOutput {
  sources?: { url: string, type?: string }[]
  groundingChunks?: GoogleGroundingChunk[]
  groundingMetadata?: { groundingChunks?: GoogleGroundingChunk[] }
}

export type ToolPart = Parameters<typeof getToolName>[0]

export function getSearchQuery(part: ToolPart): string | undefined {
  return (part.input as { query?: string } | undefined)?.query
}

export function formatToolInput(part: ToolPart): string {
  const input = part.input
  if (input == null) return ''
  return typeof input === 'string' ? input : JSON.stringify(input)
}

export function formatToolOutput(part: ToolPart): string {
  const output = part.output
  if (output == null) return ''
  return typeof output === 'string' ? output : JSON.stringify(output, null, 2)
}

export function getSources(part: ToolPart): Source[] {
  const output = part.output
  if (!output) return []

  // Anthropic: array of { url, title }
  if (Array.isArray(output)) {
    return output.filter((s: Source) => s.url).map((s: Source) => ({ url: s.url, title: s.title }))
  }

  const typed = output as SearchOutput

  // OpenAI: { sources: [{ type: 'url', url }] }
  if (typed.sources) {
    return typed.sources.filter(s => s.url).map(s => ({ url: s.url }))
  }

  // Google: grounding chunks with { web: { uri, title } }
  const chunks = typed.groundingChunks ?? typed.groundingMetadata?.groundingChunks
  if (chunks) {
    return chunks.filter(c => c.web?.uri).map(c => ({ url: c.web!.uri!, title: c.web!.title }))
  }

  return []
}

export interface SearchChunk {
  documentId: string
  title: string
  url: string
  content: string
  chunkOffset: number
  chunkLength: number
  documentTotalLength: number
  score: number
}

export interface ExtractedDocument {
  documentId: string
  title: string
  url: string
  content: string
  offset: number
  size: number
  documentTotalLength: number
}

export function getSearchQueries(part: ToolPart): string[] {
  const input = part.input as { queries?: unknown } | null | undefined
  if (!Array.isArray(input?.queries)) return []
  return input.queries.filter((q): q is string => typeof q === 'string')
}

export function getSearchChunks(part: ToolPart): SearchChunk[] {
  const output = part.output as { chunks?: unknown } | null | undefined
  if (!Array.isArray(output?.chunks)) return []

  return output.chunks.flatMap((raw): SearchChunk[] => {
    const c = raw as Record<string, unknown>
    if (!c || typeof c.content !== 'string') return []
    return [{
      documentId: typeof c.document_id === 'string' ? c.document_id : '',
      title: typeof c.title === 'string' ? c.title : '',
      url: typeof c.url === 'string' ? c.url : '',
      content: c.content,
      chunkOffset: typeof c.chunk_offset === 'number' ? c.chunk_offset : 0,
      chunkLength: typeof c.chunk_length === 'number' ? c.chunk_length : 0,
      documentTotalLength: typeof c.document_total_length === 'number' ? c.document_total_length : 0,
      score: typeof c.score === 'number' ? c.score : 0,
    }]
  })
}

export function getExtractedDocument(part: ToolPart): ExtractedDocument | undefined {
  const output = part.output as Record<string, unknown> | null | undefined
  if (!output || typeof output.content !== 'string') return undefined

  return {
    documentId: typeof output.document_id === 'string' ? output.document_id : '',
    title: typeof output.title === 'string' ? output.title : '',
    url: typeof output.url === 'string' ? output.url : '',
    content: output.content,
    offset: typeof output.offset === 'number' ? output.offset : 0,
    size: typeof output.size === 'number' ? output.size : 0,
    documentTotalLength: typeof output.document_total_length === 'number' ? output.document_total_length : 0,
  }
}
