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
utils/     → shared utilities (extract_cookie, custom Axum extractors)
```
`
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

## Language

- All documentation, code comments, commit messages, and communication must be in English.
- `TODO.md` at the project root is the single source of truth for future work items.
  If the user asks to add a task for later, add it to `TODO.md`.
  When a task is completed, check `TODO.md` and remove the corresponding entry if present.

## Service layer logging

All public methods in `server/src/services/*.rs` must have `#[instrument(skip(self))]`:

- Wraps execution in a tracing span with the method name and parameter values.
  Every `tracing::info!()` call inside automatically includes the span context
  in the log output.
- `SecretBox` is safe — its `Debug` impl redacts the value.
- Skip plain-string secrets explicitly: `#[instrument(skip(self, code, state))]`.

Exit logging (`info!`):
- `info!(key = value, status = result.status())` — result fields, `status` is ALWAYS last.
- `info!(status = "success")` — for simple success-only paths (no `Result`).
- `info!(error = ..., status = result.status())` — on error.

Do NOT add `#[instrument]` on private/helper methods — only on public entry points.
Do NOT manually log entry params — the span covers that.

Example:
```rust
use tracing::{info, instrument};

#[instrument(skip(self))]
pub async fn delete_chat(&self, chat_id: &ChatId, user_id: &UserId) -> Result<(), AppError> {
    let result = /* ... */;
    match &result {
        Ok(()) => info!(status = result.status()),
        Err(e) => info!(error = e.to_string(), status = result.status()),
    }
    result
}
```

Structured fields instead of format strings — result data goes as `key = value` in the macro,
not in the message string. The `status` field must always be the last field.

## Formatting

- After making changes to Rust code under `server/` and verifying it compiles (`cargo check`), always run `cargo fmt` in the `server/` directory.

## Code rules

- Before writing a manual implementation of anything — parsing, conversion, verification,
  algorithm selection, key extraction — search the API surface of crates you already
  depend on FIRST. Use docs.rs for the EXACT version in `Cargo.lock`, not latest.
- Prefer: a method that already does what you need over manual code; a `From`/`TryFrom`
  impl over manual field mapping; a dedicated module like `jwk` or `crypto` over ad-hoc
  structs and functions.
- If you find yourself writing string splitting, base64, manual JSON digging, or
  match-on-algorithm-enum — stop. Ask: "Does the library already do this?"
- If you've searched and genuinely found nothing suitable in any existing dependency,
  **do not implement from scratch**. Tell the user what you need and ask how to proceed.
- Applies universally across all crates and domains — JSON, JWT, SQL, HTTP, crypto, etc.

## Auth architecture

- OIDC provider: Keycloak (configurable). Backend is a confidential client, auth code flow.
- Stateless state token: `{redirect, nonce, exp}` signed with HS256 via `jsonwebtoken`.
  No server-side in-memory or DB storage for pending auth states.
- Sessions stored in `sessions` table: `provider_sid`, `access_token`, `refresh_token`,
  `expires_at`. Cleanup via periodic task (interval configurable, default 10 min).
- `AuthUser` domain type in `domain/models/auth_user.rs`: `{ user_id: UserId, session_id: String }`.
  Used as an Axum extractor (impl `FromRequestParts` in `middleware/auth.rs`).
  Injected into request extensions by `auth_middleware`.
- `AuthService` holds `OidcClient` in a `tokio::sync::OnceCell` — lazy init, OIDC server
  may be unreachable at startup.
- `verify_token(token, jwks, issuer)` is a pure function — testable without async/HTTP.
  Uses `jsonwebtoken::jwk::JwkSet::find(kid)` + `DecodingKey::try_from(jwk)`.
  Algorithm taken from token header (`header.alg`), not JWK.
- `AuthResult` enum: `Valid(AuthUser)`, `Refreshed(AuthUser, access_token)`, `Expired`, `Invalid`.
  `AuthService::verify_or_refresh` does a single session lookup by `provider_sid` from the
  access token JWT — user identity comes from `session.user_id`, not a separate `users` query.
- Refresh flow: middleware catches `Expired` → `auth_service.refresh_session` looks up
  the session by `provider_sid` from JWT's `sid` claim, refreshes via OIDC, upserts new
  tokens, returns new `AuthUser` + new `access_token`. Middleware sets `Set-Cookie` with
  `path=/api` on the response.
- CSRF middleware (`middleware/csrf.rs`): on POST/PUT/PATCH/DELETE, validates
  `x-csrf-token` header matches the `csrf-token` cookie (JWT HS256 with `{jti, sid}`),
  verifies JWT signature, and checks `sid` in JWT matches `AuthUser.session_id`.
  Returns 401 on any failure. Cookie set in OIDC callback (non-HttpOnly, path=/).
- `access_token` cookie: `HttpOnly`, `path=/api`, `SameSite=Lax`, `Secure` if HTTPS.
- `csrf-token` cookie: non-HttpOnly (JS reads it for header), `path=/`, `SameSite=Lax`.
- Cookie extraction from multi-cookie headers: shared `extract_cookie(headers, name)` in
  `utils/cookies.rs`. Iterates over `split(';')` to handle any cookie order.
- Frontend: `apiFetch` with 401 interceptor → `POST /api/auth/login` → `{loginUrl}` →
  redirect to Keycloak. Dedup via `loginPromise`.
- Session TTL: computed as `access_token_expires_in × session_ttl_multiplier` (configurable, default 10).
- Ownership checks: `chat_service::check_ownership(chat_id, user_id)` → lightweight
  `SELECT EXISTS(...)` via `chat_repository::is_owner`. Used in `send_message`, `get_votes`,
  `vote`, `delete_messages_after` handlers.

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

