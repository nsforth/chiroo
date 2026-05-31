import { createI18n } from 'vue-i18n'
import { en as uiEn, ru as uiRu } from '@nuxt/ui/locale'

import enMsg from '../locales/en.json'
import ruMsg from '../locales/ru.json'

const localeConfig = {
  en: { messages: enMsg, nuxtUi: uiEn, name: 'English' },
  ru: { messages: ruMsg, nuxtUi: uiRu, name: 'Русский' }
} as const

type LocaleCode = keyof typeof localeConfig

const browserCode = navigator.language.slice(0, 2)
const defaultLocale: LocaleCode = (browserCode in localeConfig ? browserCode : 'en') as LocaleCode

export const i18n = createI18n({
  legacy: false,
  locale: defaultLocale,
  fallbackLocale: 'en',
  messages: Object.fromEntries(
    Object.entries(localeConfig).map(([k, v]) => [k, v.messages])
  )
})

export function getLocales() {
  return Object.entries(localeConfig).map(([code, config]) => ({
    code,
    name: config.name
  }))
}

export function getNuxtUiLocale(code: string) {
  return localeConfig[code as LocaleCode]?.nuxtUi ?? uiEn
}
