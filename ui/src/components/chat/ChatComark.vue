<script setup lang="ts">
import ComarkLight from './ComarkLight'
import ComarkHighlight from './ComarkHighlight'

defineProps<{
  markdown: string
  streaming: boolean
}>()
</script>

<template>
  <ComarkLight
    v-if="streaming"
    :markdown="markdown"
    :streaming="true"
  />
  <ComarkHighlight
    v-else
    :markdown="markdown"
    :streaming="false"
  />
</template>
