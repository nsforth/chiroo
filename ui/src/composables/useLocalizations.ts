import { z } from 'zod'
import { apiFetch } from '../api'
import { i18n } from '../plugins/i18n'

const localizationBundleSchema = z.record(z.string(), z.record(z.string(), z.unknown()))

export async function loadLocalizations(): Promise<void> {
  const data: unknown = await apiFetch('/api/localizations')
  const bundles = localizationBundleSchema.parse(data)

  for (const [lang, messages] of Object.entries(bundles)) {
    i18n.global.mergeLocaleMessage(lang, messages)
  }
}
