import { isToday, isYesterday, subMonths } from 'date-fns'
import { computed, ref } from 'vue'
import { createSharedComposable } from '@vueuse/core'
import { useI18n } from 'vue-i18n'
import { $fetch } from 'ofetch'

interface ChatsResponse {
  chats: ChatResponse[]
}

interface ChatResponse {
  id: string,
  title?: string,
  userId: string,
  createdAt: string
}

interface Chat {
  id: string
  label: string
  to: string
  icon: string
  createdAt: string
}

export const useChats = createSharedComposable(() => {
  const chats = ref<Chat[]>([])
  const { t } = useI18n()

 const fetchChats = async () => {
    const data = await $fetch('/api/chats') as ChatsResponse
    chats.value = data.chats.map(chat => ({
      id: chat.id,
      label: chat.title || t('chat.untitled'),
      to: `/chat/${chat.id}`,
      icon: 'i-lucide-message-circle',
      createdAt: String(chat.createdAt)
    }) as Chat)
  }

  const updateChat = (id: string, partial: Partial<Chat>) => {
    chats.value = chats.value.map(c => c.id === id ? { ...c, ...partial } : c)
  }

  const removeChat = (id: string) => {
    chats.value = chats.value.filter(c => c.id !== id)
  }

  const groups = computed(() => {
    // Group chats by date
    const today: Chat[] = []
    const yesterday: Chat[] = []
    const lastWeek: Chat[] = []
    const lastMonth: Chat[] = []
    const older: Record<string, Chat[]> = {}

    const oneWeekAgo = subMonths(new Date(), 0.25) // ~7 days ago
    const oneMonthAgo = subMonths(new Date(), 1)

    chats.value?.forEach((chat) => {
      const chatDate = new Date(chat.createdAt)

      if (isToday(chatDate)) {
        today.push(chat)
      } else if (isYesterday(chatDate)) {
        yesterday.push(chat)
      } else if (chatDate >= oneWeekAgo) {
        lastWeek.push(chat)
      } else if (chatDate >= oneMonthAgo) {
        lastMonth.push(chat)
      } else {
        // Format: "January 2023", "February 2023", etc.
        const i18nLocale = ['ru', 'en'].includes(navigator.language.slice(0, 2)) ? navigator.language : 'en-US'
        const monthYear = chatDate.toLocaleDateString(i18nLocale, {
          month: 'long',
          year: 'numeric'
        })

        if (!older[monthYear]) {
          older[monthYear] = []
        }

        older[monthYear].push(chat)
      }
    })

    // Sort older chats by month-year in descending order (newest first)
    const sortedMonthYears = Object.keys(older).sort((a, b) => {
      const dateA = new Date(a)
      const dateB = new Date(b)
      return dateB.getTime() - dateA.getTime()
    })

    // Create formatted groups for navigation
    const formattedGroups = [] as Array<{
      id: string
      label: string
      items: Array<Chat>
    }>

    // Add groups that have chats
    if (today.length) {
      formattedGroups.push({
        id: 'today',
        label: t('groups.today'),
        items: today
      })
    }

    if (yesterday.length) {
      formattedGroups.push({
        id: 'yesterday',
        label: t('groups.yesterday'),
        items: yesterday
      })
    }

    if (lastWeek.length) {
      formattedGroups.push({
        id: 'last-week',
        label: t('groups.lastWeek'),
        items: lastWeek
      })
    }

    if (lastMonth.length) {
      formattedGroups.push({
        id: 'last-month',
        label: t('groups.lastMonth'),
        items: lastMonth
      })
    }

    // Add each month-year group
    sortedMonthYears.forEach((monthYear) => {
      if (older[monthYear]?.length) {
        formattedGroups.push({
          id: monthYear,
          label: monthYear,
          items: older[monthYear]
        })
      }
    })

    return formattedGroups
  })

  return {
    groups,
    chats,
    fetchChats,
    updateChat,
    removeChat
  }
})
