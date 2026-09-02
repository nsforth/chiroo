<script setup lang="ts">
import { computed, inject } from 'vue'
import type { Ref } from 'vue'
import { getDomain, getFaviconUrl } from '../../utils/url'
import { useBranding } from '../../composables/useBranding'

const props = defineProps<{
  href?: string
}>()

const { branding } = useBranding()

const isExternal = computed(() => /^https?:\/\//i.test(props.href || ''))
const domain = computed(() => getDomain(props.href || ''))
const favicon = computed(() => getFaviconUrl(props.href || ''))
const internalClass = computed(() => branding.value?.colors
  ? 'font-medium text-primary underline decoration-primary/50 underline-offset-2 hover:decoration-primary'
  : '')

const sourceIndex = inject<Ref<Map<string, number>> | undefined>('sourceIndex', undefined)
const num = computed(() => sourceIndex?.value?.get(props.href || '') ?? null)
</script>

<template>
  <template v-if="isExternal">
    <a
      :href="href"
      target="_blank"
      rel="noopener noreferrer"
      class="inline-flex items-center gap-1.5 align-middle px-2 py-0.5 border border-default rounded-full text-sm text-muted hover:text-default hover:bg-elevated/50 transition-colors no-underline"
    >
      <img
        :src="favicon"
        :alt="domain"
        class="size-4 shrink-0 rounded-sm"
        loading="lazy"
        @error="($event.target as HTMLImageElement).style.display = 'none'"
      >
      <span class="truncate max-w-48"><slot /></span>
      <span class="i-lucide-arrow-up-right size-3 shrink-0 opacity-60" />
    </a>
    <a
      v-if="num !== null"
      :href="href"
      target="_blank"
      rel="noopener noreferrer"
      class="align-super text-[10px] leading-none text-muted hover:text-default no-underline ms-0.5"
    >{{ num }}</a>
  </template>
  <a
    v-else
    :href="href"
    :class="internalClass"
  >
    <slot />
  </a>
</template>
