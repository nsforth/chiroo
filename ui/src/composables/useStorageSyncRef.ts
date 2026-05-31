import { watch, type Ref } from 'vue'
import { useStorage } from '@vueuse/core'

export function useStorageSyncRef<T>(key: string, ref: Ref<T>) {
  const stored = useStorage(key, ref.value)
  watch(ref, (val) => { stored.value = val })
  watch(stored, (val) => { ref.value = val })
}
