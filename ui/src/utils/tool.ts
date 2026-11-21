import { z } from 'zod'
import type { getToolName } from 'ai'

export type ToolPart = Parameters<typeof getToolName>[0]

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

const searchInputSchema = z.object({ queries: z.array(z.string()).optional() })

export function getSearchQueries(part: ToolPart): string[] {
  const parsed = searchInputSchema.safeParse(part.input ?? {})
  return parsed.success ? (parsed.data.queries ?? []) : []
}

const searchChunkSchema = z.object({
  document_id: z.string(),
  title: z.string(),
  url: z.string(),
  content: z.string(),
  chunk_offset: z.number(),
  chunk_length: z.number(),
  document_total_length: z.number(),
  score: z.number(),
  query: z.string().optional(),
})

const searchOutputSchema = z.object({ chunks: z.array(searchChunkSchema) })

export interface SearchChunk {
  documentId: string
  title: string
  url: string
  content: string
  chunkOffset: number
  chunkLength: number
  documentTotalLength: number
  score: number
  query?: string
}

export function getSearchChunks(part: ToolPart): SearchChunk[] {
  const parsed = searchOutputSchema.safeParse(part.output)
  if (!parsed.success) return []
  return parsed.data.chunks.map(c => ({
    documentId: c.document_id,
    title: c.title,
    url: c.url,
    content: c.content,
    chunkOffset: c.chunk_offset,
    chunkLength: c.chunk_length,
    documentTotalLength: c.document_total_length,
    score: c.score,
    query: c.query,
  }))
}

const extractedDocumentSchema = z.object({
  document_id: z.string(),
  title: z.string(),
  url: z.string(),
  content: z.string(),
  offset: z.number(),
  size: z.number(),
  document_total_length: z.number(),
})

export interface ExtractedDocument {
  documentId: string
  title: string
  url: string
  content: string
  offset: number
  size: number
  documentTotalLength: number
}

export function getExtractedDocument(part: ToolPart): ExtractedDocument | undefined {
  const parsed = extractedDocumentSchema.safeParse(part.output)
  if (!parsed.success) return undefined
  const d = parsed.data
  return {
    documentId: d.document_id,
    title: d.title,
    url: d.url,
    content: d.content,
    offset: d.offset,
    size: d.size,
    documentTotalLength: d.document_total_length,
  }
}
