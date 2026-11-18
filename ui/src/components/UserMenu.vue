<script setup lang="ts">
import { computed } from 'vue'
import { useColorMode } from '@vueuse/core'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import type { DropdownMenuItem } from '@nuxt/ui'
import { useUserSession } from '../composables/useUserSession'
import { getLocales } from '../plugins/i18n'

defineProps<{
  collapsed?: boolean
}>()

const router = useRouter()
const colorMode = useColorMode()
const { user, clearSession } = useUserSession()
const { t, locale } = useI18n()

const isAdmin = computed(() => user.value?.admin ?? false)

const items = computed<DropdownMenuItem[][]>(() => ([[{
  type: 'label',
    label: user.value?.name,
    avatar: {
      src: user.value?.avatar || undefined,
      alt: user.value?.name
    }
}], [{
  label: t('userMenu.language'),
  icon: 'i-lucide-languages',
  children: getLocales().map(({ code, name }) => ({
    label: name,
    type: 'checkbox' as const,
    checked: locale.value === code,
    onSelect(e: Event) {
      e.preventDefault()
      locale.value = code
    }
  }))
}, {
  label: t('userMenu.appearance'),
  icon: 'i-lucide-sun-moon',
  children: [{
    label: t('userMenu.light'),
    icon: 'i-lucide-sun',
    type: 'checkbox',
    checked: colorMode.value === 'light',
    onSelect(e: Event) {
      e.preventDefault()

      colorMode.value = 'light'
    }
  }, {
    label: t('userMenu.dark'),
    icon: 'i-lucide-moon',
    type: 'checkbox',
    checked: colorMode.value === 'dark',
    onUpdateChecked(checked: boolean) {
      if (checked) {
        colorMode.value = 'dark'
      }
    },
    onSelect(e: Event) {
      e.preventDefault()
    }
  }]
}], [
  ...(isAdmin.value ? [{
    label: t('userMenu.adminPanel'),
    icon: 'i-lucide-shield-check',
    onSelect() {
      router.push('/admin')
    }
  }] : []),
  {
    label: t('userMenu.logout'),
    icon: 'i-lucide-log-out',
    onSelect() {
      clearSession()
      router.push('/')
    }
  }
]]))
</script>

<template>
  <UDropdownMenu
    :items="items"
    :content="{ align: 'center', collisionPadding: 12 }"
    :ui="{ content: collapsed ? 'w-48' : 'w-(--reka-dropdown-menu-trigger-width)' }"
  >
    <UButton
      v-bind="{
        label: collapsed ? undefined : (user?.name || user?.username),
        trailingIcon: collapsed ? undefined : 'i-lucide-chevrons-up-down'
      }"
      :avatar="{
        src: user?.avatar || undefined,
        alt: user?.name || user?.username
      }"
      color="neutral"
      variant="ghost"
      block
      :square="collapsed"
      class="data-[state=open]:bg-elevated"
      :ui="{
        trailingIcon: 'text-dimmed'
      }"
    />
  </UDropdownMenu>
</template>
