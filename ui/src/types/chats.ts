import { z } from 'zod'
import type { UIMessage } from 'ai'

export const chatListItemSchema = z.object({
  id: z.string(),
  title: z.string().nullish(),
  userId: z.string(),
  visibility: z.enum(['public', 'private']),
  createdAt: z.string(),
})
export type ChatListItem = z.infer<typeof chatListItemSchema>

export const chatListResponseSchema = z.object({
  chats: z.array(chatListItemSchema),
})
export type ChatListResponse = z.infer<typeof chatListResponseSchema>

export const chatDetailSchema = z.object({
  id: z.string(),
  userId: z.string(),
  title: z.string().nullish(),
  visibility: z.enum(['public', 'private']),
  createdAt: z.string(),
  model: z.string(),
  // The ai SDK has no zod schema for UIMessage (deeply nested); validate the envelope, pass messages through typed
  messages: z.array(z.custom<UIMessage>(() => true)),
  isOwner: z.boolean(),
})
export type ChatDetail = z.infer<typeof chatDetailSchema>

export const messageMetadataSchema = z.object({
  scenarioId: z.string().optional(),
  name: z.string().optional(),
})
export type MessageMetadata = z.infer<typeof messageMetadataSchema>

// metadata is set server-side and is untrusted — validate it in one place
export function getMessageMetadata(message?: UIMessage): MessageMetadata | undefined {
  if (!message?.metadata) return undefined
  const parsed = messageMetadataSchema.safeParse(message.metadata)
  if (!parsed.success) {
    console.warn(`Invalid message metadata for ${message.id}:`, parsed.error)
    return undefined
  }
  return parsed.data
}
