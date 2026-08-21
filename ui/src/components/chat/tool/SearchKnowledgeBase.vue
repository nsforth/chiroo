<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import ComarkTool from '../ComarkTool'
import { getDomain, getFaviconUrl } from '../../../utils/url'
import type { SearchChunk } from '../../../utils/tool'

const { t } = useI18n()

const props = defineProps<{
  streaming: boolean
  queries: string[]
  chunks: SearchChunk[]
  errorText?: string
}>()

const text = computed(() => {
  if (props.errorText) return t('toolStatus.searchError')
  if (props.streaming) return t('toolStatus.searchingKnowledgeBase')
  return t('toolStatus.foundResults', { count: props.chunks.length })
})

const suffix = computed(() => props.queries.join(', '))
</script>

<template>
  <UChatTool
    :text="text"
    :suffix="suffix"
    icon="i-lucide-search"
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
      v-else-if="chunks.length"
      class="flex flex-col gap-2"
    >
      <div
        v-for="(chunk, index) in chunks"
        :key="`${chunk.documentId}-${chunk.chunkOffset}-${index}`"
        class="flex flex-col gap-1 border border-default rounded-md p-2"
      >
        <div class="flex items-center gap-2 min-w-0">
          <ULink
            :href="chunk.url"
            target="_blank"
            class="flex items-center gap-2 text-sm min-w-0 hover:underline"
          >
            <img
              v-if="chunk.url"
              :src="getFaviconUrl(chunk.url)"
              :alt="getDomain(chunk.url)"
              class="size-4 shrink-0 rounded-sm"
              loading="lazy"
              @error="($event.target as HTMLImageElement).style.display = 'none'"
            >
            <span class="truncate">{{ chunk.title || getDomain(chunk.url) }}</span>
          </ULink>
          <UBadge
            :label="chunk.score.toFixed(2)"
            color="neutral"
            variant="subtle"
            size="sm"
            class="ms-auto shrink-0"
          />
        </div>

        <UCollapsible>
          <template #default="{ open }">
            <UButton
              variant="link"
              color="neutral"
              size="xs"
              :label="open ? t('toolStatus.hideContent') : t('toolStatus.showContent')"
              :trailing-icon="open ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
              class="px-0 text-dimmed font-medium self-start"
            />
          </template>
          <template #content>
            <div class="max-h-64 overflow-auto">
              <ComarkTool :value="chunk.content" />
            </div>
          </template>
        </UCollapsible>
      </div>
    </div>

    <div
      v-else-if="streaming && queries.length"
      class="p-1 text-xs text-muted truncate"
    >
      {{ suffix }}
    </div>

    <div
      v-else
      class="p-1 text-sm text-muted"
    >
      {{ t('toolStatus.noResults') }}
    </div>
  </UChatTool>
</template>
