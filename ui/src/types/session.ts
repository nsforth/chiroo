import { z } from 'zod'

export const userSchema = z.object({
  id: z.string(),
  name: z.string(),
  avatar: z.string().nullish(),
  username: z.string(),
  admin: z.boolean(),
})
export type User = z.infer<typeof userSchema>

export const userSessionSchema = z.object({
  id: z.string(),
  createdAt: z.string(),
  user: userSchema.optional(),
})
export type UserSession = z.infer<typeof userSessionSchema>
