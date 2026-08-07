<script setup lang="ts">
import { useI18n } from 'vue-i18n'

defineProps<{
  toolName: string
  streaming: boolean
  inputText: string
  outputText: string
  errorText?: string
}>()

const { t } = useI18n()
</script>

<template>
  <UChatTool
    :text="streaming ? t('toolStatus.runningTool', { tool: toolName }) : t('toolStatus.completedTool', { tool: toolName })"
    :suffix="inputText"
    :streaming="streaming"
    chevron="leading"
  >
    <div
      v-if="errorText"
      class="p-1 text-sm text-error whitespace-pre-wrap break-words"
    >
      {{ errorText }}
    </div>
    <pre
      v-else-if="outputText"
      class="p-1 text-xs text-muted whitespace-pre-wrap break-words max-h-48 overflow-y-auto"
    >{{ outputText }}</pre>
  </UChatTool>
</template>
