<script setup lang="ts">
import { computed, provide } from 'vue'
import { isReasoningUIPart, isTextUIPart, isToolUIPart, getToolName } from 'ai'
import type { UIMessage } from 'ai'
import { isPartStreaming, isToolStreaming } from '@nuxt/ui/utils/ai'
import { useI18n } from 'vue-i18n'
import ChatComark from '../ChatComark.vue'
import ChatToolChart from '../tool/Chart.vue'
import ChatToolWeather from '../tool/Weather.vue'
import ChatToolSources from '../tool/Sources.vue'
import ChatToolGeneric from '../tool/Generic.vue'
import ChatMessageSources from './ChatMessageSources.vue'
import ChatMessageEdit from './MessageEdit.vue'
import { getMergedParts, getMessageSources } from '../../../utils/ai'
import { formatToolInput, formatToolOutput, getSearchQuery, getSources } from '../../../utils/tool'
import type { WeatherUIToolInvocation } from '../../../tools/weather'
import type { ChartUIToolInvocation } from '../../../tools/chart'

const { t } = useI18n()

const props = defineProps<{
  message: UIMessage
  editing: boolean
}>()

const sources = computed(() => getMessageSources(props.message))

const sourceIndex = computed(() => {
  const map = new Map<string, number>()
  sources.value.forEach((source, index) => map.set(source.url, index + 1))
  return map
})

provide('sourceIndex', sourceIndex)

const emit = defineEmits<{
  save: [message: UIMessage, text: string]
  cancelEdit: []
}>()
</script>

<template>
  <template
    v-for="(part, index) in getMergedParts(message.parts)"
    :key="`${message.id}-${part.type}-${index}`"
  >
    <UChatReasoning
      v-if="isReasoningUIPart(part)"
      :text="part.text"
      :streaming="isPartStreaming(part)"
      chevron="leading"
    >
      <ChatComark
        :markdown="part.text"
        :streaming="isPartStreaming(part)"
      />
    </UChatReasoning>

    <template v-else-if="isToolUIPart(part)">
      <ChatToolChart
        v-if="getToolName(part) === 'chart'"
        :invocation="{ ...(part as ChartUIToolInvocation) }"
      />
      <ChatToolWeather
        v-else-if="getToolName(part) === 'weather'"
        :invocation="{ ...(part as WeatherUIToolInvocation) }"
      />
      <UChatTool
        v-else-if="getToolName(part) === 'web_search' || getToolName(part) === 'google_search'"
        :text="isToolStreaming(part) ? t('toolStatus.searching') : t('toolStatus.searched')"
        :suffix="getSearchQuery(part)"
        :streaming="isToolStreaming(part)"
        chevron="leading"
      >
        <ChatToolSources :sources="getSources(part)" />
      </UChatTool>

      <ChatToolGeneric
        v-else
        :tool-name="getToolName(part)"
        :streaming="isToolStreaming(part)"
        :input-text="formatToolInput(part)"
        :output-text="formatToolOutput(part)"
        :error-text="part.errorText"
      />
    </template>

    <template v-else-if="isTextUIPart(part)">
      <ChatComark
        v-if="message.role === 'assistant'"
        :markdown="part.text"
        :streaming="isPartStreaming(part)"
      />
      <template v-else-if="message.role === 'user'">
        <ChatMessageEdit
          v-if="editing"
          :message="message"
          :text="part.text"
          @save="(msg, text) => emit('save', msg, text)"
          @cancel="emit('cancelEdit')"
        />
        <p
          v-else
          class="whitespace-pre-wrap"
        >
          {{ part.text }}
        </p>
      </template>
    </template>
  </template>

  <template v-if="message.role === 'assistant' && sources.length">
    <ChatMessageSources :sources="sources" />
  </template>
</template>
