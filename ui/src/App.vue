<script setup lang="ts">
import { computed } from 'vue'
import { useHead } from '@unhead/vue'
import { useColorMode } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { getNuxtUiLocale } from './plugins/i18n'
import { useBranding } from './composables/useBranding'
import { useStorageSyncRef } from './composables/useStorageSyncRef'
import { mixHex } from './utils/color'
import { bootFailed } from './boot'
import BootError from './components/BootError.vue'

const colorMode = useColorMode()
const { branding } = useBranding()

const palette = computed(() => branding.value?.colors ?? null)
const themeColor = computed(() => {
  const colors = palette.value
  if (!colors) {
    return colorMode.value === 'dark' ? '#09090b' : '#fafafa'
  }
  return colorMode.value === 'dark'
    ? colors.neutralDark
    : mixHex(colors.neutral, '#ffffff', 0.05)
})

const { locale } = useI18n()
useStorageSyncRef('locale', locale)
const nuxtUiLocale = computed(() => getNuxtUiLocale(locale.value))

const appConfig = useAppConfig()
if (palette.value) {
  appConfig.ui.colors.primary = 'brand'
  appConfig.ui.colors.neutral = 'ink'
}

useHead({
  meta: [
    { name: 'theme-color', content: themeColor }
  ]
})
</script>

<template>
  <UApp
    :locale="nuxtUiLocale"
    :toaster="{ position: 'top-right' }"
  >
    <BootError v-if="bootFailed" />
    <Suspense v-else>
      <RouterView />
    </Suspense>
  </UApp>
</template>
