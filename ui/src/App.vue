<script setup lang="ts">
import { computed, watch } from 'vue'
import { useHead } from '@unhead/vue'
import { useColorMode, useStorage } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { getNuxtUiLocale } from './plugins/i18n'

const colorMode = useColorMode()
const themeColor = computed(() => colorMode.value === 'dark' ? '#09090b' : '#fafafa')

const { locale } = useI18n()
const nuxtUiLocale = computed(() => getNuxtUiLocale(locale.value))

const storedLocale = useStorage('locale', locale.value)
watch(locale, (val) => { storedLocale.value = val })
watch(storedLocale, (val) => { locale.value = val })

useHead({
  meta: [
    { name: 'theme-color', content: themeColor }
  ]
})
</script>

<template>
  <Suspense>
    <UApp
      :locale="nuxtUiLocale"
      :toaster="{ position: 'top-right' }"
    >
      <RouterView />
    </UApp>
  </Suspense>
</template>
