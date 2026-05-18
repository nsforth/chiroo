import { createI18n } from 'vue-i18n'

import en from '../locales/en.json'
import ru from '../locales/ru.json'

const browserLocale = navigator.language.slice(0, 2)
const locale: string = ['ru', 'en'].includes(browserLocale) ? browserLocale : 'en'

export const i18n = createI18n({
  legacy: false,
  locale,
  fallbackLocale: 'en',
  messages: {
    en,
    ru
  }
})
