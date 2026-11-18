import { computed, ref } from 'vue'
import { createSharedComposable } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { z } from 'zod'
import { apiFetch } from '../api'
import { iconByName, type ModelOption } from '../../shared/utils/models'
import { modelSchema, type ApiModel } from '../types/models'

export const useModels = createSharedComposable(() => {
  const { t, te } = useI18n()
  const raw = ref<ApiModel[]>([])

  async function load() {
    try {
      const data: unknown = await apiFetch('/api/models')
      const parsed = z.array(modelSchema).safeParse(data)
      raw.value = parsed.success ? parsed.data : []
    } catch (error) {
      console.warn('Failed to load models:', error)
      raw.value = []
    }
  }

  void load()

  const models = computed<ModelOption[]>(() => raw.value.map((m) => {
    const labelKey = `scenarios.${m.id}.label`
    const descriptionKey = `scenarios.${m.id}.description`
    return {
      value: m.id,
      icon: iconByName(m.icon),
      iconUrl: m.iconUrl ?? undefined,
      label: te(labelKey) ? t(labelKey) : m.id,
      description: te(descriptionKey) ? t(descriptionKey) : undefined
    }
  }))

  return {
    models
  }
})
