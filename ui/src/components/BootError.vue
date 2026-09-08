<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { runBoot } from '../boot'

const { t } = useI18n()
const loading = ref(false)

async function retry() {
  if (loading.value) return
  loading.value = true
  try {
    await runBoot()
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <UError
    :error="{ statusMessage: t('errors.failedToLoad') }"
    :clear="false"
    class="min-h-screen"
  >
    <template #links>
      <UButton
        size="lg"
        :label="t('buttons.retry')"
        :loading="loading"
        @click="retry"
      />
    </template>
  </UError>
</template>
