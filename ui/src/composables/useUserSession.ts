import { createSharedComposable } from '@vueuse/core'
import { ref, computed } from 'vue'
import type { UserSession } from '../types/session'
import { apiFetch } from '../api'

export const useUserSession = createSharedComposable(() => {
  const session = ref<UserSession | null>(null)
  const loading = ref(false)

  const clearSession = async () => {
    session.value = null
  }

  const fetchSession = async () => {
    loading.value = true
    session.value = await apiFetch<UserSession>('/api/session').catch(() => null)
    loading.value = false
  }

  return {
    loggedIn: computed(() => Boolean(session.value?.user)),
    user: computed(() => session.value?.user || null),
    session,
    loading,
    fetchSession,
    clearSession,
  }
})
