import { ref, watch } from 'vue'
import { useStorage } from '@vueuse/core'

export function useChatDraft(key: string) {
  const stored = useStorage(key, '')
  const input = ref(stored.value)
  let pending: string | null = null

  watch(input, (value) => {
    if (pending === null) stored.value = value
  })
  watch(stored, (value) => {
    input.value = value
  })

  // Takes the text out of the input and keeps it in storage until the
  // request outcome is known (commit on success, restore on failure).
  function submit(): string | null {
    const text = input.value
    if (!text.trim()) return null
    pending = text
    input.value = ''
    return text
  }

  // Only clears the stored draft when the input is still empty — the user
  // may have typed a new draft while the request was in flight.
  function commit() {
    pending = null
    if (input.value === '') stored.value = ''
  }

  function restore() {
    if (pending !== null && input.value === '') {
      stored.value = pending
      input.value = pending
    }
    pending = null
  }

  return { input, submit, commit, restore }
}
