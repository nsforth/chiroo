<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'

const props = defineProps<{
  title?: string
}>()

const emit = defineEmits<{ close: [string | false] }>()
const { t } = useI18n()

const value = ref(props.title ?? '')

const trimmed = computed(() => value.value.trim())

function submit() {
  if (!trimmed.value) return
  emit('close', trimmed.value)
}
</script>

<template>
  <UModal
    :title="t('modal.renameTitle')"
    :description="t('modal.renameDesc')"
    :ui="{
      footer: 'flex-row-reverse justify-start'
    }"
    :close="false"
  >
    <template #body>
      <UInput
        v-model="value"
        autofocus
        size="lg"
        :placeholder="t('modal.chatTitlePlaceholder')"
        class="w-full"
        :ui="{ root: 'w-full' }"
        @keydown.enter.prevent="submit"
      />
    </template>

    <template #footer>
      <UButton
        :label="t('buttons.save')"
        :disabled="!trimmed"
        @click="submit"
      />
      <UButton
        color="neutral"
        variant="ghost"
        :label="t('buttons.cancel')"
        @click="emit('close', false)"
      />
    </template>
  </UModal>
</template>
