import { ref } from 'vue'
import { createSharedComposable, useWebNotification } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useBranding } from './useBranding'

export const useNotifications = createSharedComposable(() => {
  const toast = useToast()
  const { t } = useI18n()
  const { branding } = useBranding()
  const router = useRouter()

  const web = useWebNotification({ requestPermissions: false })

  const lastChatId = ref<string | null>(null)

  web.onClick(() => {
    const chatId = lastChatId.value
    web.close()
    window.focus()
    if (chatId) {
      router.push(`/chat/${chatId}`)
    }
  })

  async function request() {
    const granted = await web.ensurePermissions()
    if (!granted) {
      toast.add({
        description: t('notifications.denied'),
        icon: 'i-lucide-alert-circle',
        color: 'error'
      })
    }
  }

  function notifyChatFinished({ chatId, title }: { chatId: string, title: string | null }) {
    if (!web.isSupported.value || !web.permissionGranted.value) return
    if (!document.hidden) return

    lastChatId.value = chatId

    void web.show({
      title: title ?? t('chat.untitled'),
      body: t('notifications.answerReady'),
      tag: chatId,
      icon: branding.value?.faviconUrl
    })
  }

  return {
    isSupported: web.isSupported,
    isGranted: web.permissionGranted,
    request,
    notifyChatFinished
  }
})
