import { defineComarkComponent } from '@comark/vue'
import SourceLink from './SourceLink.vue'

export default defineComarkComponent({
  name: 'ChatComarkLight',
  plugins: [],
  components: {
    'source-link': SourceLink
  },
  class: '*:first:mt-0 *:last:mb-0'
})
