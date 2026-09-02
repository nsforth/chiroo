import { ref, watch } from 'vue'
import { createSharedComposable } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { apiFetch } from '../api'
import { iconByName, type ModelOption } from '../../shared/utils/models'

interface ApiModel {
  id: string
  icon: string
  label: string
  description: string
  iconUrl: string | null
}

export const useModels = createSharedComposable(() => {
  const { locale } = useI18n()
  const models = ref<ModelOption[]>([])

  async function load() {
    try {
      const list = await apiFetch<ApiModel[]>('/api/models', {
        query: { lang: locale.value }
      })
      models.value = list.map((m) => ({
        label: m.label,
        value: m.id,
        icon: iconByName(m.icon),
        iconUrl: m.iconUrl ?? undefined,
        description: m.description
      }))
    } catch {
      models.value = []
    }
  }

  void load()
  watch(locale, () => void load())

  return {
    models
  }
})
