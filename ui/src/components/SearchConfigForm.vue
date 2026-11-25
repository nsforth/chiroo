<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useStorage } from '@vueuse/core'
import type { ChatSearchConfigInput, ChatSearchConfigPayload } from '../types/chats'

const { t } = useI18n()

const config = useStorage<ChatSearchConfigInput>('chat-search-config', {
  mode: 'hybrid',
  limit: null,
  candidateLimit: null,
  rerankerLimit: null,
  rerankerThreshold: null,
  rerank: true
})

const modeItems = computed(() => [
  { label: t('searchConfig.modeHybrid'), value: 'hybrid' },
  { label: t('searchConfig.modeSemantic'), value: 'semantic' },
  { label: t('searchConfig.modeLexical'), value: 'lexical' }
])

// Build the request payload containing only explicit overrides so unset
// fields fall back to the server defaults.
function getSearchConfig(): ChatSearchConfigPayload | undefined {
  const c = config.value
  const out: Record<string, unknown> = {}
  if (c.mode !== 'hybrid') out.mode = c.mode
  if (c.limit !== null) out.limit = c.limit
  if (c.candidateLimit !== null) out.candidateLimit = c.candidateLimit
  if (c.rerankerLimit !== null) out.rerankerLimit = c.rerankerLimit
  if (c.rerankerThreshold !== null) out.rerankerThreshold = c.rerankerThreshold
  if (c.rerank === false) out.rerank = false
  return Object.keys(out).length > 0 ? (out as ChatSearchConfigPayload) : undefined
}

defineExpose({ getSearchConfig })
</script>

<template>
  <UCard class="w-full">
    <UCollapsible>
      <template #default="{ open }">
        <button
          type="button"
          class="flex w-full items-center gap-2 py-1 text-left text-sm"
        >
          <UIcon
            name="i-lucide-settings-2"
            class="size-4 text-dimmed"
          />
          <span class="font-medium text-highlighted">
            {{ t('searchConfig.title') }}
          </span>
          <UIcon
            :name="open ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
            class="ms-auto size-4 text-dimmed"
          />
        </button>
      </template>

      <template #content>
        <div class="mt-2 grid grid-cols-1 gap-x-6 gap-y-4 sm:grid-cols-2">
          <div class="flex flex-col gap-1.5">
            <label class="text-xs font-medium text-dimmed">
              {{ t('searchConfig.mode') }}
            </label>
            <USelect
              v-model="config.mode"
              :items="modeItems"
              size="sm"
              variant="outline"
            />
          </div>

          <div class="flex flex-col gap-1.5">
            <label class="text-xs font-medium text-dimmed">
              {{ t('searchConfig.rerank') }}
            </label>
            <div class="flex h-9 items-center">
              <USwitch
                v-model="config.rerank"
                size="sm"
                :label="t('searchConfig.rerankLabel')"
              />
            </div>
          </div>

          <div class="flex flex-col gap-1.5">
            <label class="text-xs font-medium text-dimmed">
              {{ t('searchConfig.limit') }}
            </label>
            <UInputNumber
              v-model="config.limit"
              :min="1"
              size="sm"
              :increment="false"
              :decrement="false"
              :placeholder="t('searchConfig.empty')"
            />
          </div>

          <div class="flex flex-col gap-1.5">
            <label class="text-xs font-medium text-dimmed">
              {{ t('searchConfig.candidateLimit') }}
            </label>
            <UInputNumber
              v-model="config.candidateLimit"
              :min="1"
              size="sm"
              :increment="false"
              :decrement="false"
              :placeholder="t('searchConfig.empty')"
            />
          </div>

          <div class="flex flex-col gap-1.5">
            <label class="text-xs font-medium text-dimmed">
              {{ t('searchConfig.rerankerLimit') }}
            </label>
            <UInputNumber
              v-model="config.rerankerLimit"
              :min="1"
              size="sm"
              :increment="false"
              :decrement="false"
              :placeholder="t('searchConfig.empty')"
            />
          </div>

          <div class="flex flex-col gap-1.5">
            <label class="text-xs font-medium text-dimmed">
              {{ t('searchConfig.rerankerThreshold') }}
            </label>
            <UInputNumber
              v-model="config.rerankerThreshold"
              :min="0"
              :max="1"
              :step="0.05"
              size="sm"
              :increment="false"
              :decrement="false"
              :placeholder="t('searchConfig.empty')"
            />
          </div>
        </div>

        <p class="mt-3 text-xs text-dimmed">
          {{ t('searchConfig.hint') }}
        </p>
      </template>
    </UCollapsible>
  </UCard>
</template>
