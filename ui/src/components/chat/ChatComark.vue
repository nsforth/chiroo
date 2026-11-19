<script setup lang="ts">
import { ref, shallowRef, watch } from 'vue'
import { MarkdownDocument } from '@comark/vue'
import { createSerializedMarkdownParser } from '@comark/vue/parse'
import shiki from '@comark/vue/plugins/shiki'
import html from '@shikijs/langs/html'
import css from '@shikijs/langs/css'
import python from '@shikijs/langs/python'
import sql from '@shikijs/langs/sql'
import go from '@shikijs/langs/go'
import rust from '@shikijs/langs/rust'
import java from '@shikijs/langs/java'
import c from '@shikijs/langs/c'
import cpp from '@shikijs/langs/cpp'
import ruby from '@shikijs/langs/ruby'
import php from '@shikijs/langs/php'
import swift from '@shikijs/langs/swift'
import kotlin from '@shikijs/langs/kotlin'
import diff from '@shikijs/langs/diff'
import dockerfile from '@shikijs/langs/dockerfile'
import xml from '@shikijs/langs/xml'
import toml from '@shikijs/langs/toml'
import graphql from '@shikijs/langs/graphql'
import ChatMarkdownLink from './ChatMarkdownLink.vue'

const props = withDefaults(defineProps<{
  value: string
  streaming?: boolean
  highlight?: boolean
}>(), {
  streaming: false,
  highlight: false
})

const languages = [html, css, python, sql, go, rust, java, c, cpp, ruby, php, swift, kotlin, diff, dockerfile, xml, toml, graphql]

const parseLight = createSerializedMarkdownParser({ plugins: [] })
const parseHighlight = createSerializedMarkdownParser({ plugins: [shiki({ languages })] })

const parsed = shallowRef<Awaited<ReturnType<typeof parseLight>> | undefined>(undefined)
const failed = ref(false)

watch(
  () => [props.value, props.streaming, props.highlight] as const,
  async ([markdown, streaming, highlight]) => {
    const parser = highlight && !streaming ? parseHighlight : parseLight
    try {
      parsed.value = await parser(markdown, { streaming })
      failed.value = false
    }
    catch (e) {
      console.warn('[comark] markdown parse failed, fallback to plain text', e, markdown.slice(0, 200))
      parsed.value = undefined
      failed.value = true
    }
  },
  { immediate: true }
)

const components = { a: ChatMarkdownLink }
</script>

<template>
  <pre
    v-if="failed"
    class="m-0 whitespace-pre-wrap"
  >{{ value }}</pre>
  <MarkdownDocument
    v-else
    :value="parsed"
    :components="components"
    :streaming="streaming"
    class="*:first:mt-0 *:last:mb-0"
  />
</template>
