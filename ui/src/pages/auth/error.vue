<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { loginAndRedirect } from '../../api'

const { t } = useI18n()
</script>

<template>
  <UError :error="{ statusMessage: t('errors.authFailed'), statusCode: 401 }">
    <template #links>
      <UButton
        size="lg"
        :label="t('buttons.retry')"
        @click="loginAndRedirect('/')"
      />
    </template>
  </UError>
</template>
