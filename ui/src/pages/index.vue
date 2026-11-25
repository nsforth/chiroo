<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useStorage } from '@vueuse/core'
import { apiFetch } from '../api'
import { useChats } from '../composables/useChats'
import { useChatDraft } from '../composables/useChatDraft'
import { useCsrf } from '../composables/useCsrf'
import { useUserSession } from '../composables/useUserSession'
import Navbar from '../components/Navbar.vue'
import { useBranding } from '../composables/useBranding'
import { chatListItemSchema } from '../types/chats'
import type { ChatSearchConfigPayload } from '../types/chats'
import SearchConfigForm from '../components/SearchConfigForm.vue'

const { branding } = useBranding()

const { fetchChats } = useChats()
const { csrf, headerName } = useCsrf()
const model = useStorage<string>('model', 'simple')
const { user } = useUserSession()
const toast = useToast()
const { t } = useI18n()
const { input, submit, commit, restore } = useChatDraft('draft.new-chat')
const loading = ref(false)
const router = useRouter()
const searchConfigForm = ref<InstanceType<typeof SearchConfigForm> | null>(null)

const greeting = computed(() => {
  const hour = new Date().getHours()
  let timeGreeting = t('greeting.evening')
  if (hour < 12) timeGreeting = t('greeting.morning')
  else if (hour < 18) timeGreeting = t('greeting.afternoon')

  const name = user.value?.name?.split(' ')[0] || user.value?.username

  return name ? `${timeGreeting}, ${name}` : `${timeGreeting}`
})

async function createChat(prompt: string) {
  loading.value = true
  try {
    const searchConfig = searchConfigForm.value?.getSearchConfig()
    const body: {
      firstMessage: string
      model: string
      searchConfig?: ChatSearchConfigPayload
    } = { firstMessage: prompt, model: model.value }
    if (searchConfig) body.searchConfig = searchConfig

    const data: unknown = await apiFetch('/api/chats', {
      method: 'POST',
      headers: { [headerName]: csrf() },
      body
    })
    const parsed = chatListItemSchema.safeParse(data)
    if (!parsed.success) throw new Error('invalid response')

    commit()
    await fetchChats()
    router.push(`/chat/${parsed.data.id}`)
  } catch {
    restore()
    toast.add({
      description: t('errors.failedToCreateChat'),
      icon: 'i-lucide-alert-circle',
      color: 'error'
    })
  } finally {
    loading.value = false
  }
}

function onSubmit() {
  if (loading.value) return
  const text = submit()
  if (text === null) return
  void createChat(text)
}
</script>

<template>
  <UDashboardPanel
    id="home"
    class="min-h-0"
    :ui="{ body: 'p-0 sm:p-0' }"
  >
    <template #header>
      <Navbar />
    </template>

    <template #body>
      <UContainer class="flex-1 flex flex-col justify-center gap-4 sm:gap-6 py-8">
        <div class="flex items-center gap-4">
          <img
            v-if="branding?.assistantUrl"
            :src="branding.assistantUrl"
            :alt="branding.name"
            class="size-15 shrink-0 rounded-2xl object-contain"
          >
          <h1 class="text-3xl sm:text-4xl text-highlighted font-bold">
            {{ greeting }}
          </h1>
        </div>

        <UChatPrompt
          v-model="input"
          :placeholder="t('placeholders.chatInputPrompt')"
          class="[view-transition-name:chat-prompt]"
          variant="subtle"
          :ui="{ base: 'px-1.5' }"
          @submit="onSubmit"
        >
          <template #footer>
            <div class="ms-auto flex items-center gap-3">
              <ModelSelect v-model="model" />

              <UChatPromptSubmit
                :status="loading ? 'streaming' : 'ready'"
                color="neutral"
                size="sm"
              />
            </div>
          </template>
        </UChatPrompt>

        <SearchConfigForm ref="searchConfigForm" />
      </UContainer>
    </template>
  </UDashboardPanel>
</template>
