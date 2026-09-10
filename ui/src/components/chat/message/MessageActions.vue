<script setup lang="ts">
import { ref, computed } from 'vue'
import type { UIMessage } from 'ai'
import { isFileUIPart } from 'ai'
import { useClipboard } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { getMessageText } from '../../../utils/ai'
import { useModels } from '../../../composables/useModels'

const { t } = useI18n()
const { models } = useModels()

const props = defineProps<{
  message: UIMessage & { createdAt?: string | Date }
  streaming: boolean
  editing: boolean
  vote: boolean | null
  showDetails: boolean
  readonly?: boolean
}>()

const scenarioName = computed(() => {
  const metadata = props.message.metadata as { name?: string } | undefined
  return metadata?.name ?? null
})

const scenario = computed(() => models.value.find((m) => m.value === scenarioName.value))

const formattedDate = computed(() => {
  if (!props.message.createdAt) return null

  const date = new Date(props.message.createdAt)

  return {
    time: date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' }),
    full: date.toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' }),
    iso: date.toISOString()
  }
})

const emit = defineEmits<{
  edit: [message: UIMessage]
  regenerate: [message: UIMessage]
  vote: [message: UIMessage, isUpvoted: boolean]
}>()

const hasFiles = computed(() => props.message.parts.some(isFileUIPart))

const clipboard = useClipboard()

const copied = ref(false)

function copy() {
  clipboard.copy(getMessageText(props.message, props.showDetails))

  copied.value = true

  setTimeout(() => {
    copied.value = false
  }, 2000)
}
</script>

<template>
  <template v-if="message.role === 'assistant' && !streaming && scenarioName">
    <span class="inline-flex items-center gap-1.5 text-xs text-muted mr-1.5 max-w-40">
      <img
        v-if="scenario?.iconUrl"
        :src="scenario.iconUrl"
        :alt="scenario.label"
        class="size-4 rounded-sm object-contain"
        loading="lazy"
      >
      <UIcon
        v-else
        :name="scenario?.icon ?? 'i-lucide-bot'"
        class="size-4 shrink-0"
      />
      <UTooltip :text="scenario?.description">
        <span class="truncate">{{ scenario?.label ?? scenarioName }}</span>
      </UTooltip>
    </span>
  </template>

  <template v-if="!streaming && !editing">
    <UTooltip :text="t('chatActions.copyResponse')">
      <UButton
        size="sm"
        :color="copied ? 'primary' : 'neutral'"
        variant="ghost"
        :icon="copied ? 'i-lucide-copy-check' : 'i-lucide-copy'"
        :aria-label="t('chatActions.copyResponse')"
        @click="copy"
      />
    </UTooltip>
  </template>

  <template v-if="message.role === 'assistant' && !streaming && !readonly">
    <UTooltip :text="t('chatActions.goodResponse')">
      <UButton
        size="sm"
        :color="vote === true ? 'success' : 'neutral'"
        variant="ghost"
        icon="i-lucide-thumbs-up"
        :aria-label="t('chatActions.goodResponse')"
        @click="emit('vote', message, true)"
      />
    </UTooltip>

    <UTooltip :text="t('chatActions.badResponse')">
      <UButton
        size="sm"
        :color="vote === false ? 'error' : 'neutral'"
        variant="ghost"
        icon="i-lucide-thumbs-down"
        :aria-label="t('chatActions.badResponse')"
        @click="emit('vote', message, false)"
      />
    </UTooltip>

    <UTooltip :text="t('chatActions.regenerate')">
      <UButton
        size="sm"
        color="neutral"
        variant="ghost"
        icon="i-lucide-rotate-cw"
        :aria-label="t('chatActions.regenerate')"
        @click="emit('regenerate', message)"
      />
    </UTooltip>
  </template>

  <template v-if="message.role === 'user' && !streaming && !editing">
    <UTooltip
      v-if="formattedDate"
      :text="formattedDate.full"
    >
      <time
        :datetime="formattedDate.iso"
        class="text-xs text-muted mr-1.5"
      >
        {{ formattedDate.time }}
      </time>
    </UTooltip>

    <UTooltip
      v-if="!readonly && !hasFiles"
      :text="t('chatActions.editMessage')"
    >
      <UButton
        size="sm"
        color="neutral"
        variant="ghost"
        icon="i-lucide-pencil"
        :aria-label="t('chatActions.editMessage')"
        @click="emit('edit', message)"
      />
    </UTooltip>
  </template>
</template>
