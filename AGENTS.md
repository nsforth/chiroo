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

## AI service (`services/ai_service.rs`)

### Dependencies

`async-openai = { version = "0.40", features = ["chat-completion", "byot"] }`

`AiService` owns `Client<OpenAIConfig>` and a model name. It spawns a tokio task per request: LLM loop with tool execution, event accumulation, SSE forwarding. Handler only maps `StreamEvent → SseChunk → SSE`.

### `byot` (Bring Your Own Types)

async-openai's standard `ChatCompletionStreamResponseDelta` does **not** include vendor-specific fields like `reasoning_content` (Qwen, DeepSeek). `byot` feature generates `create_stream_byot<T, R>` — you provide your **own** response type `R: DeserializeOwned + MaybeSend + 'static`:

```rust
#[derive(Deserialize)]
struct CustomDelta {
    content: Option<String>,
    #[serde(rename = "reasoning_content", default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChatCompletionMessageToolCallChunk>>,
}

#[derive(Deserialize)]
struct CustomChoice {
    delta: CustomDelta,
    finish_reason: Option<FinishReason>,
}

#[derive(Deserialize)]
struct CustomChunk {
    choices: Vec<CustomChoice>,
}

type CustomStream = Pin<Box<dyn Stream<Item = Result<CustomChunk, OpenAIError>> + Send>>;

let mut stream: CustomStream = client.chat().create_stream_byot(request).await?;
```

**Critical**: `create_stream` (non-byot) forces `stream: Some(true)` on the request before sending. `create_stream_byot` does NOT — the request param is generic `T0: Serialize`. You MUST call `.stream(true)` on the request builder explicitly.

### Tool-call loop

Uses a labeled `'step: for _step in 0..max_steps` loop. The `continue 'step;` label is required because `continue` inside nested `for choice` / `while stream` would only continue the innermost loop. Without the label, control falls through to `return Ok(())` after the inner stream exhausts, silently terminating the LLM loop after one tool call.

### Control flow

```
AiService.stream(chat_id, user_text) → mpsc channel → tokio::spawn:
  └─ build OpenAI request with tools
  └─ 'step: loop (max_steps)
       └─ send LlmCallStart
       └─ create_stream_byot → iterate chunks
            ├─ reasoning_content → ReasoningDelta
            ├─ content → TextDelta
            ├─ tool_calls → ToolCallStart/Delta
            └─ finish_reason:
                 ├─ ToolCalls → execute tools → continue 'step
                 └─ Stop → send LlmCallFinish → return
```

