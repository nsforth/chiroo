<script setup lang="ts">
import { useBranding } from '../composables/useBranding'

defineProps<{
  collapsed: boolean
}>()

const { branding } = useBranding()
</script>

<template>
  <ULink
    v-if="!collapsed"
    to="/"
    class="flex items-center gap-2 min-w-0"
  >
    <img
      :src="branding?.logoUrl"
      :alt="branding?.name"
      class="h-7 w-auto shrink-0"
    >
    <span class="flex flex-col min-w-0 leading-tight">
      <span
        v-if="branding?.label != null"
        class="text-xl font-bold text-highlighted truncate"
      >{{ branding?.label }}</span>
      <span
        v-if="branding?.tagline"
        class="text-xs text-muted truncate"
      >{{ branding.tagline }}</span>
    </span>
  </ULink>

  <UDashboardSidebarCollapse class="ms-auto" />
</template>
