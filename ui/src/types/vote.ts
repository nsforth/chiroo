import { z } from 'zod'

export const voteSchema = z.object({
  chatId: z.string(),
  scenarioId: z.string(),
  isUpvoted: z.boolean(),
})
export type Vote = z.infer<typeof voteSchema>
