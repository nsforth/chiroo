# Chiroo — Agent Context

## Stack

- Frontend: Vue 3 + Vite, Nuxt UI + Tailwind CSS, Vercel AI SDK, vue-i18n
- Backend: Rust + Axum (Tokio), Diesel ORM, PostgreSQL
- Vite dev proxy: `localhost:5143/api/*` → `localhost:3000`

## API conventions

- JSON camelCase: `userId`, `createdAt`, `firstMessage`, `messageId`, `toolCallId`
- All dates are RFC 3339 strings

## Backend architecture (DDD)

```
handlers/  → HTTP layer, converts domain ↔ requests/responses/DTOs
services/  → business logic, orchestrates repositories
infra/     → Diesel schema, repositories (SQL queries)
domain/    → pure data types (Chat, ChatId, UserId, Vote, MessageId)
```

`AppState` is the DI container holding all services.

## Diesel / Database

- Always use `NOT NULL` unless the column is genuinely optional.
- Use `JSON` not `JSONB`.

## SSE / AI SDK stream format

Wire format:
```
data: <JSON>\n\n
data: <JSON>\n\n
...
```

Backend returns `Content-Type: text/event-stream` via `axum::response::sse::Sse`.

Frontend pipeline:
```
ReadableStream → TextDecoderStream → EventSourceParserStream
  → JSON.parse → strictObject schema validation → processUIMessageStream
```

**All chunks must pass `z.strictObject` schema — extra fields break validation.**

`[DONE]` is optional. Stream ends when HTTP response body closes.

## SSE chunk registry

| Chunk | Key fields |
|-------|-----------|
| `start` | `messageId?` |
| `finish` | `finishReason?` (`"stop"`, `"length"`, `"content-filter"`, `"tool-calls"`, `"error"`, `"other"`) |
| `start-step` | (none) |
| `finish-step` | (none) |
| `text-start` / `text-delta` / `text-end` | `id`, `delta` |
| `reasoning-start` / `reasoning-delta` / `reasoning-end` | `id`, `delta` |
| `tool-input-start` | `toolCallId`, `toolName`, `title?`, `providerExecuted?` |
| `tool-input-delta` | `toolCallId`, `inputTextDelta` |
| `tool-input-available` | `toolCallId`, `toolName`, `input`, `providerExecuted?` |
| `tool-output-available` | `toolCallId`, `output`, `providerExecuted?` |
| `tool-output-error` | `toolCallId`, `errorText` |
| `source-url` | `sourceId`, `url`, `title?` |
| `source-document` | `sourceId`, `mediaType`, `title` |
| `file` | `url`, `mediaType` |
| `data-*` | `id?`, `data` (e.g. `data-chat-title`) |
| `error` | `errorText` |
| `abort` | `reason?` |

## Steps (start-step / finish-step)

- Step = one LLM invocation. Multiple steps = tool-call loop.
- `start-step` → pushes `{ type: "step-start" }` into `message.parts[]` (invisible boundary).
- `finish-step` → resets `activeTextParts`/`activeReasoningParts`, does NOT produce a part.
- Simple text response: 1 step. Tool calls: 1 step per LLM iteration.
- During `convertToModelMessages`, `step-start` parts split the flat `parts[]` into separate
  assistant ModelMessage blocks per step.

## Tools

- Frontend does NOT auto-execute tools. `onToolCall` is undefined in Chat constructor.
- Set `providerExecuted: true` on all server-side tool chunks.
- Frontend tool rendering: `MessageContent.vue`, switch by `getToolName(part)`.
- Each tool component expects a specific output shape — mismatch causes
  `TypeError` during render, potentially freezing the UI.

### Weather tool output shape

```json
{
  "location": "string",
  "temperature": 18,
  "temperatureHigh": 22,
  "temperatureLow": 12,
  "condition": { "text": "Cloudy", "icon": "i-lucide-cloud" },
  "humidity": 72,
  "windSpeed": 15,
  "dailyForecast": [
    { "day": "Today", "high": 22, "low": 14, "condition": { "text": "...", "icon": "..." } }
  ]
}
```

### Chart tool output shape (vue-chrts LineChart)

```json
{
  "title": "string?",
  "data": [{ "xKey": "...", "seriesKey": 42 }],
  "xKey": "string",
  "series": [{ "key": "seriesKey", "name": "Series", "color": "#3b82f6" }],
  "xLabel": "string?",
  "yLabel": "string?"
}
```

## Frontend flow

- Home page (`index.vue`): `createChat()` → `POST /api/chats { firstMessage: prompt }`
  → `router.push('/chat/{id}')`
- Chat page (`[id].vue`):
  1. `GET /api/chats/{id}` → loads chat + messages
  2. `new Chat({ id, messages, transport: DefaultChatTransport({ api: '/api/chats/{id}', body: { model } }) })`
  3. `onMounted`: if `messages.length === 1`, calls `chat.regenerate()`
- `chat.regenerate()`: truncates to last user message, POSTs `{ trigger: "regenerate-message" }`
- `chat.sendMessage({ text })`: appends user text, POSTs `{ trigger: "submit-message" }`

## Known issues / gotchas

- CSRF: frontend sends `x-csrf-token` header, backend does NOT validate it. Do not try to fix this, it's in TODO list.
- Auth: user UUID is hardcoded (`b78f01ac-5225-11f1-a097-e71b54ed4087`). Also do not try to fix it.

