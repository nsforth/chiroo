import { defineAsyncComponent } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { $fetch } from 'ofetch'
import { useChats } from './useChats'
import { useCsrf } from './useCsrf'

const ModalRename = defineAsyncComponent(() => import('../components/ModalRename.vue'))
const ModalConfirm = defineAsyncComponent(() => import('../components/ModalConfirm.vue'))

export function useChatActions() {
  const route = useRoute()
  const router = useRouter()
  const toast = useToast()
  const overlay = useOverlay()
  const { t } = useI18n()
  const { csrf, headerName } = useCsrf()
  const { updateChat, removeChat } = useChats()

  const renameModal = overlay.create(ModalRename)
  const deleteModal = overlay.create(ModalConfirm, {
    props: {
      title: t('modal.deleteTitle'),
      description: t('modal.deleteDesc'),
      color: 'error'
    }
  })

  async function renameChat(id: string, currentTitle?: string | null): Promise<string | null> {
    const instance = renameModal.open({ title: currentTitle ?? '' })
    const result = await instance.result

    if (!result || result === currentTitle) return null

    try {
      await $fetch(`/api/chats/title/${id}`, {
        method: 'PATCH',
        headers: { [headerName]: csrf() },
        body: { title: result }
      })

      updateChat(id, { label: result })

      return result
    } catch {
      toast.add({
        description: t('errors.failedToRenameChat'),
        icon: 'i-lucide-alert-circle',
        color: 'error'
      })

      return null
    }
  }

  async function deleteChat(id: string): Promise<boolean> {
    const instance = deleteModal.open()
    const result = await instance.result

    if (!result) return false

    try {
      await $fetch(`/api/chats/${id}`, {
        method: 'DELETE',
        headers: { [headerName]: csrf() }
      })

      toast.add({
        title: t('toast.chatDeleted'),
        description: t('toast.chatDeletedDesc'),
        icon: 'i-lucide-trash'
      })

      removeChat(id)

      if ((route.params as { id?: string }).id === id) {
        router.push('/')
      }

      return true
    } catch {
      toast.add({
        description: t('errors.failedToDeleteChat'),
        icon: 'i-lucide-alert-circle',
        color: 'error'
      })

      return false
    }
  }

  return {
    renameChat,
    deleteChat
  }
}
