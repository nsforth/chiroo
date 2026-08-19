<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { apiFetch } from '../api'
import { useChats } from '../composables/useChats'
import { useCsrf } from '../composables/useCsrf'
import { useUserSession } from '../composables/useUserSession'
import Navbar from '../components/Navbar.vue'

const { fetchChats } = useChats()
const { csrf, headerName } = useCsrf()
const { user } = useUserSession()
const toast = useToast()
const { t } = useI18n()
const input = ref('')
const loading = ref(false)
const router = useRouter()

const greeting = computed(() => {
  const hour = new Date().getHours()
  let timeGreeting = t('greeting.evening')
  if (hour < 12) timeGreeting = t('greeting.morning')
  else if (hour < 18) timeGreeting = t('greeting.afternoon')

  const name = user.value?.name?.split(' ')[0] || user.value?.username

  return name ? `${timeGreeting}, ${name}` : `${timeGreeting}`
})

async function createChat(prompt: string) {
  input.value = prompt
  loading.value = true
  try {
    const chat = await apiFetch('/api/chats', {
      method: 'POST',
      headers: { [headerName]: csrf() },
      body: { firstMessage: prompt }
    })

    await fetchChats()
    router.push(`/chat/${chat?.id}`)
  } catch {
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
  createChat(input.value)
}

const quickChats = ref<{ label: string; icon: string }[]>([])
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
        <h1 class="text-3xl sm:text-4xl text-highlighted font-bold">
          {{ greeting }}
        </h1>

        <UChatPrompt
          v-model="input"
          :status="loading ? 'streaming' : 'ready'"
          :placeholder="t('placeholders.chatInputPrompt')"
          class="[view-transition-name:chat-prompt]"
          variant="subtle"
          :ui="{ base: 'px-1.5' }"
          @submit="onSubmit"
        >
          <template #footer>
            <div class="ms-auto flex items-center gap-3">
              <ModelSelect />

              <UChatPromptSubmit
                color="neutral"
                size="sm"
              />
            </div>
          </template>
        </UChatPrompt>

        <div class="flex flex-wrap gap-2">
          <UButton
            v-for="quickChat in quickChats"
            :key="quickChat.label"
            :icon="quickChat.icon"
            :label="quickChat.label"
            size="sm"
            color="neutral"
            variant="outline"
            class="rounded-full"
            @click="createChat(quickChat.label)"
          />
        </div>
      </UContainer>
    </template>
  </UDashboardPanel>
</template>
