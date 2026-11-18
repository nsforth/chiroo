<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { apiFetch } from '../../api'
import { format, parseISO } from 'date-fns'
import { votePageSchema, type VoteAdmin } from '../../types/votes'
import type { TableColumn } from '@nuxt/ui'

definePage({ meta: { layout: 'admin' } })

const { t } = useI18n()
const toast = useToast()

const page = ref(1)
const pageSize = 25
const items = ref<VoteAdmin[]>([])
const total = ref(0)
const loading = ref(true)
const forbidden = ref(false)

async function load() {
  loading.value = true
  try {
    const data: unknown = await apiFetch(`/api/admin/votes?page=${page.value}&pageSize=${pageSize}`)
    const parsed = votePageSchema.safeParse(data)
    if (!parsed.success) {
      toast.add({
        description: t('admin.votes.loadError'),
        icon: 'i-lucide-alert-circle',
        color: 'error'
      })
      return
    }
    items.value = parsed.data.items
    total.value = parsed.data.total
    forbidden.value = false
  } catch (e) {
    const status = (e as { status?: number })?.status ?? (e as { statusCode?: number })?.statusCode
    if (status === 403) {
      forbidden.value = true
    } else {
      toast.add({
        description: t('admin.votes.loadError'),
        icon: 'i-lucide-alert-circle',
        color: 'error'
      })
    }
  } finally {
    loading.value = false
  }
}

onMounted(load)
watch(page, () => { void load() })

function formatDate(rfc3339: string) {
  try { return format(parseISO(rfc3339), 'yyyy-MM-dd HH:mm') } catch { return rfc3339 }
}

function getRowId(row: VoteAdmin): string {
  return row.scenarioId
}

const columns = computed<TableColumn<VoteAdmin>[]>(() => [
  { accessorKey: 'user', header: t('admin.votes.colUser') },
  { accessorKey: 'votedAt', header: t('admin.votes.colDate') },
  { accessorKey: 'model', header: t('admin.votes.colModel') },
  { accessorKey: 'isUpvoted', header: t('admin.votes.colVote') },
  { accessorKey: 'chatTitle', header: t('admin.votes.colChat') },
])
</script>

<template>
  <UDashboardPanel
    id="adminPanel"
    class="min-h-0"
  >
    <template #header>
      <UDashboardNavbar
        class="absolute top-0 inset-x-0 border-b-0 z-10 backdrop-blur lg:backdrop-blur-none pointer-events-none sm:px-4"
        :ui="{ left: 'pointer-events-auto min-w-0', right: 'pointer-events-auto' }"
      >
        <template #left>
          <span class="text-lg font-semibold">{{ t('admin.title') }}</span>
        </template>
        <template #right>
          <UColorModeButton />
        </template>
      </UDashboardNavbar>
    </template>

    <template #body>
      <UContainer class="flex-1 overflow-auto p-0 sm:p-0">
        <div
          v-if="forbidden"
          class="flex flex-1 flex-col items-center justify-center gap-2 p-8 text-center"
        >
          <UIcon
            name="i-lucide-shield-off"
            class="size-10 text-muted"
          />
          <p class="text-muted">
            {{ t('admin.votes.forbidden') }}
          </p>
          <UButton
            to="/"
            variant="link"
            :label="t('buttons.backToHome')"
          />
        </div>

        <div
          v-else
          class="flex flex-col gap-4 p-4"
        >
          <UTable
            v-if="!loading"
            :data="items"
            :columns="columns"
            :get-row-id="getRowId"
            :empty="t('admin.votes.empty')"
          >
            <template #user-cell="{ row }">
              <div class="flex items-center gap-2.5">
                <UAvatar
                  :src="row.original.user.avatar || undefined"
                  :alt="row.original.user.name || row.original.user.username"
                  class="size-8 shrink-0"
                />
                <div class="flex min-w-0 flex-col leading-tight">
                  <span class="truncate font-medium">{{ row.original.user.name || row.original.user.username }}</span>
                  <span class="text-xs text-muted">@{{ row.original.user.username }}</span>
                </div>
              </div>
            </template>
            <template #votedAt-cell="{ row }">
              <span class="text-muted">{{ formatDate(row.original.votedAt) }}</span>
            </template>
            <template #model-cell="{ row }">
              <ModelBadge :model="row.original.model" />
            </template>
            <template #chatTitle-cell="{ row }">
              <ULink
                :to="`/chat/${row.original.chatId}`"
                target="_blank"
              >
                {{ row.original.chatTitle ?? t('admin.votes.untitled') }}
              </ULink>
            </template>
            <template #isUpvoted-cell="{ row }">
              <UBadge
                :color="row.original.isUpvoted ? 'success' : 'error'"
                :icon="row.original.isUpvoted ? 'i-lucide-thumbs-up' : 'i-lucide-thumbs-down'"
                variant="soft"
                size="sm"
              >
                {{ row.original.isUpvoted ? t('admin.votes.like') : t('admin.votes.dislike') }}
              </UBadge>
            </template>
          </UTable>

          <div
            v-if="loading"
            class="flex items-center justify-center py-10"
          >
            <UIcon
              name="i-lucide-loader-circle"
              class="size-6 animate-spin text-muted"
            />
          </div>
          <div
            v-if="total > pageSize"
            class="flex justify-end"
          >
            <UPagination
              v-model:page="page"
              :total="total"
              :items-per-page="pageSize"
            />
          </div>
        </div>
      </UContainer>
    </template>
  </UDashboardPanel>
</template>
