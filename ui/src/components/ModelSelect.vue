<script setup lang="ts">
import { useModels } from '../composables/useModels'
import { useI18n } from 'vue-i18n'

const { model, models } = useModels()

const { t } = useI18n()
const searchInputAttrs = {
      placeholder: t('placeholders.modelSelectSearch'),
      icon: 'i-lucide-search'
    }
</script>

<template>
  <USelectMenu
    v-model="model"
    :items="models"
    size="sm"    
    :icon="models.find(m => m.value === model)?.icon"
    variant="ghost"
    value-key="value"
    :search-input="searchInputAttrs"
    class="data-[state=open]:bg-elevated"
    :ui="{
      trailingIcon: 'group-data-[state=open]:rotate-180 transition-transform duration-200'
    }"
  />
</template>
