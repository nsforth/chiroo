import { z } from 'zod'
import { userSchema } from './session'

export const voteAdminSchema = z.object({
  user: userSchema,
  chatId: z.string(),
  scenarioId: z.string(),
  votedAt: z.string(),
  chatTitle: z.string().nullish(),
  chatCreatedAt: z.string(),
  model: z.string(),
  isUpvoted: z.boolean(),
})
export type VoteAdmin = z.infer<typeof voteAdminSchema>

export const votePageSchema = z.object({
  items: z.array(voteAdminSchema),
  total: z.number(),
})
export type VotePage = z.infer<typeof votePageSchema>
