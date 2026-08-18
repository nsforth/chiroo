<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import type { MessageSource } from '../../../utils/ai'
import { getDomain } from '../../../utils/url'

const { t } = useI18n()

const props = defineProps<{
  sources: MessageSource[]
}>()

const LIMIT = 5
const expanded = ref(false)

const visibleSources = computed(() => (expanded.value ? props.sources : props.sources.slice(0, LIMIT)))
const hiddenCount = computed(() => props.sources.length - LIMIT)
const showMore = computed(() => hiddenCount.value > 0)
</script>

<template>
  <div>
    <p class="text-xs font-medium text-dimmed mb-1">
      {{ t('chat.sources') }}
    </p>

    <ul class="flex flex-col gap-1">
      <li
        v-for="(source, index) in visibleSources"
        :key="source.sourceId"
      >
        <ULink
          :href="source.url"
          target="_blank"
          class="flex items-center gap-2 text-sm rounded-md px-1 py-1 hover:bg-elevated/50"
        >
          <span class="w-4 shrink-0 flex justify-end">
            <UBadge
              :label="index + 1"
              color="neutral"
              variant="subtle"
              size="sm"
              square
              :ui="{ base: 'rounded-full' }"
            />
          </span>
          <span class="truncate">{{ source.title || getDomain(source.url) }}</span>
        </ULink>
      </li>
    </ul>

    <div
      v-if="showMore"
      class="flex items-center gap-2 px-1"
    >
      <span class="w-4 shrink-0" />
      <UButton
        variant="link"
        color="neutral"
        size="xs"
        :trailing-icon="expanded ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
        :label="expanded ? t('chat.sourcesShowLess') : t('chat.sourcesShowMore', { count: hiddenCount })"
        class="text-dimmed font-medium px-0"
        @click="expanded = !expanded"
      />
    </div>
  </div>
</template>
