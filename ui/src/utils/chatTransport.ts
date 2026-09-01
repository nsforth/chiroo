import type { UIMessage } from 'ai'

interface BuildSendMessageBodyInput {
  trigger: 'submit-message' | 'regenerate-message'
  messageId?: string
  messages: UIMessage[]
  model: string
  scenarioId?: string
}

export function buildSendMessageBody({ trigger, messageId, messages, model, scenarioId }: BuildSendMessageBodyInput) {
  const resolvedTrigger =
    trigger === 'regenerate-message' && !messageId ? 'continue' : trigger

  const lastUser = [...messages].reverse().find(m => m.role === 'user')
  const text = (lastUser?.parts ?? [])
    .filter(p => p.type === 'text')
    .map(p => (p as { text: string }).text)
    .join('')

  return {
    trigger: resolvedTrigger,
    messageId,
    ...(trigger === 'submit-message' && !messageId ? { model } : {}),
    text,
    userMessageId: lastUser?.id,
    ...(scenarioId ? { scenarioId } : {})
  }
}
