import { z } from 'zod'

const hexColor = z.string().regex(/^#[0-9a-fA-F]{3}([0-9a-fA-F]{3})?$/)

export const fontDescriptorSchema = z.object({
  family: z.string(),
  url: z.string(),
  format: z.string().nullable(),
})

export const paletteSchema = z.object({
  primary: hexColor,
  neutral: hexColor,
  primaryDark: hexColor,
  neutralDark: hexColor,
})

export const brandingSchema = z.object({
  name: z.string(),
  label: z.string().nullable(),
  tagline: z.string().nullable(),
  assistantName: z.string(),
  colors: paletteSchema.nullable(),
  logoUrl: z.string(),
  faviconUrl: z.string(),
  assistantUrl: z.string(),
  fontNormal: fontDescriptorSchema,
  fontItalic: fontDescriptorSchema.nullable(),
  fontMono: fontDescriptorSchema,
})

export type FontDescriptor = z.infer<typeof fontDescriptorSchema>
export type BrandingPalette = z.infer<typeof paletteSchema>
export type Branding = z.infer<typeof brandingSchema>
