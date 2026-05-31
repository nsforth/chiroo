<script setup lang="ts">
import { computed } from 'vue'
import { useHead } from '@unhead/vue'
import { useColorMode } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { getNuxtUiLocale } from './plugins/i18n'

const colorMode = useColorMode()
const themeColor = computed(() => colorMode.value === 'dark' ? '#09090b' : '#fafafa')

const { locale } = useI18n()

useHead({
  meta: [
    { name: 'theme-color', content: themeColor }
  ]
})
</script>

<template>
  <Suspense>
    <UApp
      :locale="getNuxtUiLocale(locale)"
      :toaster="{ position: 'top-right' }"
    >
      <RouterView />
    </UApp>
  </Suspense>
</template>
