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

const items = computed(() => models.value.map((model) => model.value))
</script>

<template>
  <USelectMenu
    v-model="value"
    :items="items"
    size="sm"
    variant="ghost"
    :search-input="false"
    class="data-[state=open]:bg-elevated"
    :ui="{
      trailingIcon: 'group-data-[state=open]:rotate-180 transition-transform duration-200'
    }"
  >
    <template #default>
      <ModelBadge :model="modelValue" />
    </template>

    <template #item-label="{ item }">
      <ModelBadge :model="item" />
    </template>
  </USelectMenu>
</template>
