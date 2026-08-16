import { ref, watch } from 'vue'
import { useStorage } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { apiFetch } from '../api'
import { iconByName, type ModelOption } from '../../shared/utils/models'

interface ApiModel {
  id: string
  icon: string
  label: string
  description: string
}

export function useModels() {
  const { locale } = useI18n()
  const model = useStorage<string>('model', 'simple')
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
        description: m.description
      }))
    } catch {
      models.value = []
    }
  }

  void load()
  watch(locale, () => void load())

  return {
    models,
    model
  }
}
