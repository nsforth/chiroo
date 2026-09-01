<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { UIMessage } from 'ai'
import ChatIndicator from './Indicator.vue'
import { isStreamingStatusVisible, resolveStreamingStatus } from '../../utils/streamingStatus'

const { t } = useI18n()

const props = defineProps<{
  status: string
  messages: UIMessage[]
}>()

const visible = computed(() => isStreamingStatusVisible(props.status))

const text = computed(() => {
  const status = resolveStreamingStatus(props.status, props.messages)
  if (!status) return ''

  switch (status.type) {
    case 'sending-request':
      return t('chat.sendingRequest')
    case 'thinking':
      return t('chat.thinking')
    case 'answering':
      return t('chat.answering')
    case 'searching':
      return t('chat.searching', { query: status.query })
    case 'reading-document':
      return t('chat.readingDocument')
    case 'read-document':
      return t('chat.readDocument', { title: status.title })
    case 'running-tool':
      return t('toolStatus.runningTool', { tool: status.tool })
    case 'generating-chart':
      return t('toolStatus.generatingChart')
    case 'loading-weather':
      return t('toolStatus.loadingWeather')
    default:
      return ''
  }
})
</script>

<template>
  <div
    v-if="visible"
    class="flex items-center gap-1.5"
  >
    <ChatIndicator />
    <UChatShimmer
      :text="text"
      class="text-sm"
    />
  </div>
</template>
