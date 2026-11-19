<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { apiFetch, loginAndRedirect } from '../../api'
import { Chat } from '@ai-sdk/vue'
import { DefaultChatTransport } from 'ai'
import type { UIMessage } from 'ai'
import { useI18n } from 'vue-i18n'
import { useChats } from '../../composables/useChats'
import { useCsrf } from '../../composables/useCsrf'
import { useNotifications } from '../../composables/useNotifications'
import { useUserSession } from '../../composables/useUserSession'
import { useRoute } from 'vue-router'
import ChatMessageContent from '../../components/chat/message/MessageContent.vue'
import ChatMessageActions from '../../components/chat/message/MessageActions.vue'
import ChatVisibility from '../../components/chat/ChatVisibility.vue'
import ChatTitle from '../../components/chat/ChatTitle.vue'
import ChatStreamingStatus from '../../components/chat/StreamingStatus.vue'
import Navbar from '../../components/Navbar.vue'
import { voteSchema, type Vote } from '../../types/vote'
import { chatDetailSchema, getMessageMetadata } from '../../types/chats'
import { z } from 'zod'
import { buildSendMessageBody } from '../../utils/chatTransport'
import { isStreamingStatusVisible } from '../../utils/streamingStatus'

const route = useRoute<'/chat/[id]'>()
const toast = useToast()
const { t } = useI18n()
const { fetchChats, chats } = useChats()
const { csrf, headerName } = useCsrf()
const { user } = useUserSession()
const { notifyChatFinished } = useNotifications()

const rawData: unknown = await apiFetch(`/api/chats/${route.params.id}`).catch(() => null)
const parsedChat = chatDetailSchema.safeParse(rawData)
const data = parsedChat.success ? parsedChat.data : null

const model = ref<string>(data?.model ?? 'simple')

const isOwner = computed(() => data?.isOwner ?? false)
const isAdmin = computed(() => user.value?.admin ?? false)
const visibility = ref<'public' | 'private'>(data?.visibility ?? 'private')
const title = ref<string | null>(data?.title ?? null)

watch(() => chats.value.find(c => c.id === data?.id)?.label, (label) => {
  if (label) {
    title.value = label
  }
})

const votes = ref<Vote[]>([])
if (isOwner.value) {
  apiFetch<unknown>(`/api/chats/votes/${route.params.id}`).then((v) => {
    const parsed = z.array(voteSchema).safeParse(v)
    if (parsed.success) {
      votes.value = parsed.data
    }
  }).catch(() => {})
}

const input = ref('')

const chat = new Chat({
  id: data?.id,
  messages: data?.messages,
  generateId: () => crypto.randomUUID(),
  transport: new DefaultChatTransport({
    api: `/api/chats/${data?.id}`,
    headers: { [headerName]: csrf() },
    prepareSendMessagesRequest: ({ trigger, messageId, messages, body }) => ({
      body: buildSendMessageBody({
        trigger,
        messageId,
        messages,
        model: model.value,
        scenarioId: body?.scenarioId
      })
    })
  }),
  onData: (dataPart) => {
    if (dataPart.type === 'data-chat-title') {
      fetchChats().catch(() => {})
    }
  },
  onFinish: () => {
    if (!data?.id) return
    notifyChatFinished({ chatId: data.id, title: title.value })
  },
  onError(error) {
    if (error.message?.includes('401') || error.message?.includes('Unauthorized')) {
      loginAndRedirect()
      return
    }

    let message = error.message
    if (typeof message === 'string' && message[0] === '{') {
      try {
        message = JSON.parse(message).message || message
      } catch {
        // keep original message on malformed JSON
      }
    }

    toast.add({
      description: message,
      icon: 'i-lucide-alert-circle',
      color: 'error',
      duration: 0
    })
  }
})

function handleSubmit(e: Event) {
  e.preventDefault()
  if (input.value.trim()) {
    chat.sendMessage({
      text: input.value
    })
    input.value = ''
  }
}

const statusVisible = computed(() => isStreamingStatusVisible(chat.status))

const editingMessageId = ref<string | null>(null)

function startEdit(message: UIMessage) {
  if (editingMessageId.value) return

  editingMessageId.value = message.id
}

function cancelEdit() {
  editingMessageId.value = null
}

function getScenarioId(message?: UIMessage): string | undefined {
  return getMessageMetadata(message)?.scenarioId
}

function getFollowingScenarioId(message: UIMessage): string | undefined {
  const index = chat.messages.findIndex(m => m.id === message.id)
  return getScenarioId(chat.messages[index + 1])
}

async function saveEdit(message: UIMessage, text: string) {
  editingMessageId.value = null
  const scenarioId = getScenarioId(message) ?? getFollowingScenarioId(message)
  if (!scenarioId) return
  chat.sendMessage({ text, messageId: message.id }, { body: { scenarioId } })
}

function regenerateMessage(message: UIMessage) {
  const scenarioId = getScenarioId(message)
  if (!scenarioId) return
  chat.regenerate({ messageId: message.id, body: { scenarioId } })
}

function getVote(message: UIMessage) {
  const scenarioId = getScenarioId(message)
  if (!scenarioId) return null
  const vote = votes.value.find(v => v.scenarioId === scenarioId)
  if (!vote) return null
  return !!vote.isUpvoted
}

async function vote(message: UIMessage, isUpvoted: boolean) {
  const scenarioId = getScenarioId(message)
  if (!scenarioId) return
  const snapshot = votes.value.map(v => ({ ...v }))
  const toggling = getVote(message) === isUpvoted
  const next = toggling ? null : isUpvoted

  votes.value = next === null
    ? votes.value.filter(v => v.scenarioId !== scenarioId)
    : [
        ...votes.value.filter(v => v.scenarioId !== scenarioId),
        { chatId: data!.id, scenarioId, isUpvoted: next }
      ]

  try {
    await apiFetch(`/api/chats/votes/${data!.id}`, {
      method: 'POST',
      headers: { [headerName]: csrf() },
      body: next === null ? { scenarioId } : { scenarioId, isUpvoted: next }
    })
  } catch {
    votes.value = snapshot
    toast.add({
      description: t('errors.failedToSaveVote'),
      icon: 'i-lucide-alert-circle',
      color: 'error'
    })
  }
}

onMounted(() => {
  if (isOwner.value && data?.messages?.length === 1) {
    chat.regenerate()
  }
})
</script>

<template>
  <UDashboardPanel
    v-if="data?.id"
    id="chat"
    class="relative min-h-0"
    :ui="{ body: 'p-0 sm:p-0 overscroll-none' }"
  >
    <template #header>
      <Navbar>
        <template #title>
          <ChatTitle
            :chat-id="data!.id"
            :title="title"
            :is-owner="isOwner"
            @update:title="title = $event"
          />
        </template>

        <ChatVisibility
          v-if="isOwner"
          :chat-id="data!.id"
          :visibility="visibility"
          @update:visibility="visibility = $event"
        />
      </Navbar>
    </template>

    <template #body>
      <UContainer class="flex-1 flex flex-col gap-4 sm:gap-6">
        <UChatMessages
          should-auto-scroll
          :messages="chat.messages"
          :status="chat.status"
          :spacing-offset="isOwner ? (statusVisible ? 188 : 160) : 0"
          class="pb-4 sm:pb-6"
        >
          <template #indicator />

          <template #content="{ message }">
            <ChatMessageContent
              :message="message"
              :streaming="chat.status === 'streaming' && message.id === chat.messages[chat.messages.length - 1]?.id"
              :show-details="isAdmin"
              :editing="isOwner && editingMessageId === message.id"
              @save="saveEdit"
              @cancel-edit="cancelEdit"
            />
          </template>

          <template #actions="{ message }">
            <ChatMessageActions
              :message="message"
              :streaming="chat.status === 'streaming' && message.id === chat.messages[chat.messages.length - 1]?.id"
              :editing="editingMessageId === message.id"
              :readonly="!isOwner"
              :vote="getVote(message)"
              :show-details="isAdmin"
              @edit="startEdit"
              @regenerate="regenerateMessage"
              @vote="vote"
            />
          </template>
        </UChatMessages>

        <div
          v-if="isOwner"
          class="sticky bottom-0 z-10"
        >
          <ChatStreamingStatus
            :status="chat.status"
            :messages="chat.messages"
            class="px-1.5 pb-2"
          />

          <UChatPrompt
            v-model="input"
            :placeholder="t('placeholders.chatInputPrompt')"
            :error="chat.error"
            variant="subtle"
            class="rounded-b-none [view-transition-name:chat-prompt]"
            :ui="{ base: 'px-1.5' }"
            @submit="handleSubmit"
          >
            <template #footer>
              <div class="ms-auto flex items-center gap-3">
                <ModelSelect v-model="model" />

                <UChatPromptSubmit
                  :status="chat.status"
                  color="neutral"
                  size="sm"
                  @stop="chat.stop()"
                  @reload="chat.regenerate()"
                />
              </div>
            </template>
          </UChatPrompt>
        </div>
      </UContainer>
    </template>
  </UDashboardPanel>

  <UContainer
    v-else
    class="flex-1 flex flex-col gap-4 sm:gap-6"
  >
    <UError
      :error="{ statusMessage: t('chat.notFound'), statusCode: 404 }"
      class="min-h-full"
    >
      <template #links>
        <UButton
          to="/"
          size="lg"
          :label="t('buttons.backToHome')"
        />
      </template>
    </UError>
  </UContainer>
</template>
