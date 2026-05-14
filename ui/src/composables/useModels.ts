import { useStorage } from '@vueuse/core'
import { MODELS } from '../../shared/utils/models'

export function useModels() {
  const model = useStorage<string>('model', 'LocalLLM')

  return {
    models: MODELS,
    model
  }
}
