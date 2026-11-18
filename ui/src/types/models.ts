import { z } from 'zod'

export const modelSchema = z.object({
  id: z.string(),
  icon: z.string(),
  iconUrl: z.string().nullish(),
})
export type ApiModel = z.infer<typeof modelSchema>
