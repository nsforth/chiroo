export interface FontDescriptor {
  family: string
  url: string
  format: string | null
}

export interface BrandingPalette {
  primary: string
  neutral: string
  primaryDark: string
  neutralDark: string
}

export interface Branding {
  name: string
  tagline?: string
  assistantName: string
  colors: BrandingPalette | null
  logoUrl: string
  faviconUrl: string
  assistantUrl: string
  fontSans: FontDescriptor | null
  fontMono: FontDescriptor | null
}
