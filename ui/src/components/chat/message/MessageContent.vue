<script setup lang="ts">
import { computed, provide } from 'vue'
import { isReasoningUIPart, isTextUIPart, isToolUIPart, getToolName } from 'ai'
import type { UIMessage } from 'ai'
import { isPartStreaming, isToolStreaming } from '@nuxt/ui/utils/ai'
import ChatComark from '../ChatComark.vue'
import ChatToolChart from '../tool/Chart.vue'
import ChatToolGeneric from '../tool/Generic.vue'
import ChatToolSearchKnowledgeBase from '../tool/SearchKnowledgeBase.vue'
import ChatToolExtractDocument from '../tool/ExtractDocument.vue'
import ChatMessageSources from './ChatMessageSources.vue'
import ChatMessageEdit from './MessageEdit.vue'
import { getMessageSources, getRenderableParts } from '../../../utils/ai'
import { formatToolInput, formatToolOutput, getSearchQueries, getSearchChunks, getExtractedDocument } from '../../../utils/tool'
import type { ChartUIToolInvocation } from '../../../tools/chart'

const props = defineProps<{
  message: UIMessage
  editing: boolean
  streaming: boolean
  showDetails: boolean
}>()

const sources = computed(() => getMessageSources(props.message))

const hasTextPart = computed(() => props.message.parts.some((part) => isTextUIPart(part)))

const showSources = computed(() =>
  props.message.role === 'assistant'
    && sources.value.length > 0
    && (!props.streaming || hasTextPart.value)
)

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
    v-for="(part, index) in getRenderableParts(message, showDetails)"
    :key="`${message.id}-${part.type}-${index}`"
  >
    <UChatReasoning
      v-if="showDetails && isReasoningUIPart(part)"
      :text="part.text"
      :streaming="isPartStreaming(part)"
      chevron="leading"
    >
      <ChatComark
        :markdown="part.text"
        :streaming="isPartStreaming(part)"
      />
    </UChatReasoning>

    <template v-else-if="showDetails && isToolUIPart(part)">
      <ChatToolChart
        v-if="getToolName(part) === 'chart'"
        :invocation="{ ...(part as ChartUIToolInvocation) }"
      />
      <ChatToolSearchKnowledgeBase
        v-else-if="getToolName(part) === 'search_in_all_kbs' || getToolName(part) === 'search_in_kb' || getToolName(part) === 'search_knowledge_base'"
        :streaming="isToolStreaming(part)"
        :queries="getSearchQueries(part)"
        :chunks="getSearchChunks(part)"
        :error-text="part.errorText"
      />
      <ChatToolExtractDocument
        v-else-if="getToolName(part) === 'extract_document'"
        :streaming="isToolStreaming(part)"
        :document="getExtractedDocument(part)"
        :error-text="part.errorText"
      />

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

  <template v-if="showSources">
    <ChatMessageSources :sources="sources" />
  </template>
</template>
