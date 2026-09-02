<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { loadBranding } from '../composables/useBranding'

const { t } = useI18n()
const loading = ref(false)

async function retry() {
  if (loading.value) return
  loading.value = true
  try {
    await loadBranding()
    window.location.reload()
  } catch {
    loading.value = false
  }
}
</script>

<template>
  <div class="min-h-screen flex flex-col items-center justify-center gap-6 p-6">
    <div class="flex flex-col items-center gap-2 text-center">
      <div class="text-sm text-zinc-500 dark:text-zinc-400">
        {{ t('errors.failedToLoadBranding') }}
      </div>
      <button
        type="button"
        :disabled="loading"
        class="mt-2 rounded-full border border-zinc-300 dark:border-zinc-700 px-4 py-1.5 text-sm text-zinc-700 dark:text-zinc-200 transition-colors hover:bg-zinc-100 disabled:opacity-50 dark:hover:bg-zinc-800"
        @click="retry"
      >
        {{ loading ? '...' : t('buttons.retry') }}
      </button>
    </div>
  </div>
</template>
