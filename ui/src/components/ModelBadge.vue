<script setup lang="ts">
import { computed } from 'vue'
import { useModels } from '../composables/useModels'

const props = defineProps<{ model: string }>()

const { models } = useModels()

const selected = computed(() => models.value.find((m) => m.value === props.model))
</script>

<template>
  <UTooltip :text="selected?.description">
    <span class="inline-flex items-center gap-1.5">
      <img
        v-if="selected?.iconUrl"
        :src="selected.iconUrl"
        :alt="selected.label"
        class="size-4 rounded-sm object-contain"
        loading="lazy"
      >
      <UIcon
        v-else
        :name="selected?.icon ?? 'i-lucide-bot'"
        class="size-4 shrink-0"
      />
      <span class="truncate">{{ selected?.label ?? model }}</span>
    </span>
  </UTooltip>
</template>
