import { createI18n } from 'vue-i18n'
import { en as uiEn, ru as uiRu } from '@nuxt/ui/locale'

import enMsg from '../locales/en.json'
import ruMsg from '../locales/ru.json'

const localeConfig = {
  en: { messages: enMsg, nuxtUi: uiEn },
  ru: { messages: ruMsg, nuxtUi: uiRu }
} as const

export type LocaleCode = keyof typeof localeConfig
export const localeCodes = Object.keys(localeConfig) as LocaleCode[]

const browserCode = navigator.language.slice(0, 2)
const defaultLocale: LocaleCode = localeCodes.includes(browserCode as LocaleCode)
  ? (browserCode as LocaleCode)
  : 'en'

export const i18n = createI18n({
  legacy: false,
  locale: defaultLocale,
  fallbackLocale: 'en',
  messages: Object.fromEntries(
    Object.entries(localeConfig).map(([k, v]) => [k, v.messages])
  )
})

export function getNuxtUiLocale(code: string) {
  return localeConfig[code as LocaleCode]?.nuxtUi ?? uiEn
}
