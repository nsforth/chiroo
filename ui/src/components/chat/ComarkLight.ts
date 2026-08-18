import { defineComarkComponent } from '@comark/vue'
import ChatMarkdownLink from './ChatMarkdownLink.vue'

export default defineComarkComponent({
  name: 'ChatComarkLight',
  plugins: [],
  components: {
    a: ChatMarkdownLink
  },
  class: '*:first:mt-0 *:last:mb-0'
})
