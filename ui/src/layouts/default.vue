<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import type { DropdownMenuItem } from '@nuxt/ui'
import { useChats } from '../composables/useChats'
import { useUserSession } from '../composables/useUserSession'
import { useChatActions } from '../composables/useChatActions'

const router = useRouter()
const route = useRoute()
const toast = useToast()
const { t } = useI18n()
const { loggedIn, fetchSession } = useUserSession()
const { groups, fetchChats } = useChats()
const { renameChat, deleteChat } = useChatActions()

if (route.path !== '/auth/error') {
  await fetchSession()
  try {
    await fetchChats()
  } catch {
    toast.add({
      description: t('errors.failedToLoadChats'),
      icon: 'i-lucide-alert-circle',
      color: 'error'
    })
  }
}

const sidebarOpen = ref(false)
const searchOpen = ref(false)

watch(loggedIn, () => {
  fetchChats().catch((error) => {console.log("error loading chats on login: " + error)})

  sidebarOpen.value = false
})

const items = computed(() => groups.value?.flatMap((group) => {
  return [{
    label: group.label,
    type: 'label' as const
  }, ...group.items.map(item => ({
    ...item,
    slot: 'chat' as const,
    icon: undefined,
    label: item.label ?? t('chat.untitled'),
    class: item.label ? '' : 'text-muted'
  }))]
}))

const searchGroups = computed(() => groups.value?.map(group => ({
  ...group,
  items: group.items.map(item => ({
    ...item,
    label: item.label ?? t('chat.untitled')
  }))
})) ?? [])

function getChatActions(item: { id: string, label: string | null }): DropdownMenuItem[][] {
  return [[
    {
      label: t('buttons.rename'),
      icon: 'i-lucide-pencil',
      onSelect: () => renameChat(item.id, item.label ?? '')
    }
  ], [
    {
      label: t('buttons.delete'),
      icon: 'i-lucide-trash',
      color: 'error' as const,
      onSelect: () => deleteChat(item.id)
    }
  ]]
}

defineShortcuts({
  meta_o: () => {
    router.push('/')
  }
})
</script>

<template>
  <div v-if="route.path === '/auth/error'" class="min-h-screen flex items-center justify-center">
    <RouterView />
  </div>
  <UDashboardGroup v-else unit="rem">
    <UDashboardSidebar
      id="default"
      v-model:open="sidebarOpen"
      :min-size="12"
      collapsible
      resizable
      class="border-r-0 py-4"
    >
      <template #header="{ collapsed }">
        <ULink
          v-if="!collapsed"
          to="/"
          class="flex items-center gap-0.5"
        >
          <img
            src="/logo.png"
            alt="Logo"
            class="h-7 w-auto shrink-0"
          >
          <span class="text-xl font-bold text-highlighted">Chiroo</span>
        </ULink>

        <UDashboardSidebarCollapse class="ms-auto" />
      </template>

      <template #default="{ collapsed }">
        <UNavigationMenu
          :items="[{
            label: t('buttons.newChat'),
            to: '/',
            kbds: ['meta', 'o'],
            icon: 'i-lucide-circle-plus'
          }, {
            label: t('buttons.search'),
            icon: 'i-lucide-search',
            kbds: ['meta', 'k'],
            onSelect: () => {
              searchOpen = true
            }
          }]"
          :collapsed="collapsed"
          orientation="vertical"
        >
          <template #item-trailing="{ item }">
            <div
              v-if="item.kbds?.length"
              class="flex items-center gap-px opacity-0 group-hover:opacity-100 transition-opacity"
            >
              <UKbd
                v-for="kbd in item.kbds"
                :key="kbd"
                :value="kbd"
                size="sm"
                variant="soft"
                class="bg-accented/50"
              />
            </div>
          </template>
        </UNavigationMenu>

        <UNavigationMenu
          v-if="!collapsed"
          :items="items"
          :collapsed="collapsed"
          orientation="vertical"
          :ui="{
            link: 'overflow-hidden pr-7.5',
            linkTrailing: 'translate-x-full group-hover:translate-x-0 group-has-data-[state=open]:translate-x-0 transition-transform ms-0 absolute inset-e-px'
          }"
        >
          <template #chat-trailing="{ item }">
            <UDropdownMenu
              :items="getChatActions(item as { id: string, label: string | null })"
              :content="{ align: 'end' }"
            >
              <UButton
                as="div"
                icon="i-lucide-ellipsis"
                color="neutral"
                variant="link"
                size="sm"
                class="rounded-[5px] hover:bg-accented/50 focus-visible:bg-accented/50 data-[state=open]:bg-accented/50"
                :aria-label="t('chatActions.chatActions')"
                tabindex="-1"
                @click.stop.prevent
              />
            </UDropdownMenu>
          </template>
        </UNavigationMenu>
      </template>

      <template #footer="{ collapsed }">
        <UserMenu
          v-if="loggedIn"
          :collapsed="collapsed"
        />
      </template>
    </UDashboardSidebar>

    <UDashboardSearch
      v-model:open="searchOpen"
      :color-mode="false"
      
      :groups="[{
        id: 'links',
        items: [{
          label: t('buttons.newChat'),
          to: '/',
          icon: 'i-lucide-circle-plus'
        }]
      }, ...searchGroups]"
    />

    <div class="flex-1 flex m-4 lg:ml-0 rounded-lg ring ring-default bg-default/75 shadow min-w-0 overflow-hidden">
      <RouterView :key="route.path" />
    </div>
  </UDashboardGroup>
</template>
