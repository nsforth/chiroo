<script setup lang="ts">
import { computed } from 'vue'
import { useModels } from '../composables/useModels'

const props = defineProps<{ modelValue: string }>()
const emit = defineEmits<{ 'update:modelValue': [value: string] }>()

const { models } = useModels()

const value = computed({
  get: () => props.modelValue,
  set: (v) => emit('update:modelValue', v)
})

const selected = computed(() => models.value.find((m) => m.value === value.value))

const items = computed(() => models.value.map(({ label, value, icon }) => ({ label, value, icon })))

function descriptionOf(value: string): string | undefined {
  return models.value.find((m) => m.value === value)?.description
}
</script>

<template>
  <USelectMenu
    v-model="value"
    :items="items"
    size="sm"
    :icon="selected?.icon"
    variant="ghost"
    value-key="value"
    :search-input="false"
    class="data-[state=open]:bg-elevated"
    :ui="{
      trailingIcon: 'group-data-[state=open]:rotate-180 transition-transform duration-200'
    }"
  >
    <template #default>
      <UTooltip :text="selected?.description">
        <span class="truncate">{{ selected?.label ?? modelValue }}</span>
      </UTooltip>
    </template>

    <template #item-label="{ item }">
      <UTooltip :text="descriptionOf(item.value)">
        <span class="truncate">{{ item.label }}</span>
      </UTooltip>
    </template>
  </USelectMenu>
</template>
