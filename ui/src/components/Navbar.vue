<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useNotifications } from '../composables/useNotifications'

const { t } = useI18n()
const { isSupported, isGranted, request } = useNotifications()
</script>

<template>
  <UDashboardNavbar
    class="absolute top-0 inset-x-0 border-b-0 z-10 backdrop-blur lg:backdrop-blur-none pointer-events-none sm:px-4"
    :ui="{ left: 'pointer-events-auto min-w-0', right: 'pointer-events-auto' }"
  >
    <template #left>
      <slot name="title" />
    </template>

    <template #right>
      <slot />

      <UTooltip
        v-if="isSupported && !isGranted"
        :text="t('notifications.bell')"
      >
        <UButton
          color="neutral"
          variant="ghost"
          icon="i-lucide-bell-off"
          :aria-label="t('notifications.bell')"
          @click="request()"
        />
      </UTooltip>

      <UColorModeButton />

      <UButton
        color="neutral"
        variant="ghost"
        icon="i-lucide-circle-plus"
        to="/"
        class="lg:hidden"
        :aria-label="t('buttons.newChat')"
      />
    </template>
  </UDashboardNavbar>
</template>
