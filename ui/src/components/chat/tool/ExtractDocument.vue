<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import ChatComark from '../ChatComark.vue'
import { getDomain, getFaviconUrl } from '../../../utils/url'
import type { ExtractedDocument } from '../../../utils/tool'

const { t } = useI18n()

const props = defineProps<{
  streaming: boolean
  document?: ExtractedDocument
  errorText?: string
}>()

const text = computed(() => {
  if (props.errorText) return t('toolStatus.readError')
  if (props.streaming) return t('toolStatus.readingDocument')
  return t('toolStatus.readDocument')
})

const suffix = computed(() => props.document?.title ?? '')
</script>

<template>
  <UChatTool
    :text="text"
    :suffix="suffix"
    icon="i-lucide-file-text"
    chevron="leading"
    :streaming="streaming"
    :unmount-on-hide="true"
  >
    <div
      v-if="errorText"
      class="p-1 text-sm text-error whitespace-pre-wrap break-words"
    >
      {{ errorText }}
    </div>

    <div
      v-else-if="document"
      class="flex flex-col gap-2"
    >
      <ULink
        v-if="document.url"
        :href="document.url"
        target="_blank"
        class="flex items-center gap-2 text-sm min-w-0"
      >
        <img
          :src="getFaviconUrl(document.url)"
          :alt="getDomain(document.url)"
          class="size-4 shrink-0 rounded-sm"
          loading="lazy"
          @error="($event.target as HTMLImageElement).style.display = 'none'"
        >
        <span class="truncate">{{ document.title || getDomain(document.url) }}</span>
        <span
          v-if="document.title"
          class="text-xs text-dimmed shrink-0"
        >{{ getDomain(document.url) }}</span>
      </ULink>

      <ChatComark
        :value="document.content"
        class="text-xs text-muted"
      />
    </div>
  </UChatTool>
</template>
