<script setup lang="ts">
import { ref, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useUserSession } from '../composables/useUserSession'

const route = useRoute()
const router = useRouter()
const { t } = useI18n()
const { fetchSession, loggedIn, user } = useUserSession()

await fetchSession()

if (!user.value?.admin) {
  router.replace('/')
}

const sidebarOpen = ref(false)

const navItems = computed(() => [
  { label: t('admin.nav.votes'), to: '/admin', icon: 'i-lucide-thumbs-up' }
])
</script>

<template>
  <UDashboardGroup unit="rem">
    <UDashboardSidebar
      id="admin"
      v-model:open="sidebarOpen"
      :min-size="12"
      collapsible
      resizable
      class="border-r-0 py-4"
    >
      <template #header="{ collapsed }">
        <SidebarHeader :collapsed="collapsed" />
      </template>

      <template #default="{ collapsed }">
        <UNavigationMenu
          :items="navItems"
          :collapsed="collapsed"
          orientation="vertical"
        />
      </template>

      <template #footer="{ collapsed }">
        <UserMenu
          v-if="loggedIn"
          :collapsed="collapsed"
        />
      </template>
    </UDashboardSidebar>

    <div class="flex-1 flex m-4 lg:ml-0 rounded-lg ring ring-default bg-default/75 shadow min-w-0 overflow-hidden">
      <RouterView :key="route.path" />
    </div>
  </UDashboardGroup>
</template>
