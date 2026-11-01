# Chiroo — Agent Context

## Stack

- Frontend: Vue 3 + Vite, Nuxt UI + Tailwind CSS, Vercel AI SDK, vue-i18n
- Backend: Rust + Axum (Tokio), Diesel ORM, PostgreSQL
- Vite dev proxy: `localhost:5143/api/*` → `localhost:3000`

## API conventions

- JSON camelCase: `userId`, `createdAt`, `firstMessage`, `messageId`, `toolCallId`, `scenarioId`
- All dates are RFC 3339 strings

## Backend architecture (DDD)

```
handlers/  → HTTP layer, converts domain ↔ requests/responses/DTOs
services/  → business logic, orchestrates repositories
infra/     → Diesel schema, repositories (SQL queries)
domain/    → pure data types (Chat, ChatId, UserId, Vote, Scenario, Round, MessageId)
utils/     → shared utilities (extract_cookie, custom Axum extractors)
```
`AppState` is the DI container holding all services.

## Configuration

Configuration uses the `config` crate (dependency renamed `appconfig` to avoid a
clash with `mod config`), deserialized directly into the runtime `Config` tree —
the only raw DTO is `RawBranding` (branding section, see below) — resolved from
three layers, lowest → highest precedence:

1. **Built-in defaults** — `server/config/default.yaml`, embedded via
   `include_str!`. This is the single source of truth for all non-secret
   defaults: never duplicate a default value in code.
2. **A config file** — passed with `-c` / `--config` (format from the extension).
3. **Environment variables** — `CHIROO_<SECTION>__<FIELD>` (`__` separates section
   from field, `_` stays inside a field), including anything in `server/.env`.

Flow (`src/config.rs`): `build_with_env` stacks the three layers and calls
`try_deserialize::<Config>()`. Because layer 1 always provides every default key,
fields with a default are concrete types (`String`, `u16`, `Duration`); only
genuinely optional keys (`qdrant.api_key`) are `Option` with `#[serde(default)]`.
A missing required key is a startup error
(`missing configuration field "<section>.<field>"`).

Custom (de)serialization is used only where serde can't express the logic:

- `de_db_key` — 32-byte length check → `Arc<SecretBox<[u8; KEY_LEN]>>`.
- `de_branding` — `RawBranding` → `BrandingConfig`; the only struct with a
  conversion, because it loads fonts/assets from disk and validates the palette
  (both-or-neither, hex).
- Secrets use `secrecy`'s native `Deserialize` (`SecretString`).
- `Duration` fields use `#[serde(with = "humantime_serde")]` (the serde impl for
  `std::time::Duration` only reads the `{secs, nanos}` struct form). Values are
  human-readable duration strings: `"30s"`, `"600s"`, `"5m"`, `"1h"`. A bare
  number is a startup error.

Derived values are computed getters, never stored fields
(`OidcConfig::redirect_uri`, `OidcConfig::secure_cookie`). The resolved `Config`
is held in a `OnceLock` (`load_config -> &'static Config`). The runtime
`Config`/sub-struct getters are the only surface the services, middleware and
clients touch — keep them stable.

Adding a setting = a field on the struct (+ `#[serde(rename)]` when the field
name differs from the config key) and, if it has a default, a line in
`default.yaml`.

- Env values arrive as plain strings; the `config` deserializer coerces them to
  the target type (numbers/bools parse from their string form). Do **not**
  enable `Environment::try_parsing` — it converts purely-numeric values in
  `String` fields to integers and breaks deserialization (e.g.
  `CHIROO_OPENAI__MODEL=7`). There is **no** `ignore_empty` — an empty string is
  meaningful (e.g. `CHIROO_BRANDING__LABEL=""`). Note: an empty string for a
  *required* field deserializes fine and fails later at use time (only the db
  key is validated).
- `Config` has `#[serde(deny_unknown_fields)]`, so an unknown top-level section
  (a typo like `CHIROO_SERVR__PORT`, or a stray `CHIROO_*` var from another
  service) is a startup error, not a silent default. The env source only ingests
  `CHIROO_*` keys, so unrelated vars (`PATH`, …) never reach the deserializer.
  Unknown keys *inside* a section are still ignored (the attribute is on
  `Config` only, not the sub-structs) — add it there too if a section must be
  strict.
- Secrets have no built-in value (secrets can't be in the embedded default);
  they come from the `-c` file or env. There are no cross-defaults:
  embedding/rerank endpoints must be set explicitly.

## Diesel / Database

- Always use `NOT NULL` unless the column is genuinely optional.
- Use `JSON` not `JSONB`.

## Scenarios & rounds (domain + storage)

A chat turn is a `Scenario`; the user message is the first round. Domain types in `domain/models/scenario.rs`:

- `Scenario { id: ScenarioId, chat_id, name, status, rounds: Vec<Round>, usage: Option<TokenUsage>, created_at, updated_at }`
  - `status: Pending | Running | Completed | Failed`
  - `rounds[0]` is always the user message (`Message` with role `User`); `rounds[1..]` are assistant rounds.
- `Round { message: Message, sources: Vec<Source>, hidden: bool, usage: Option<TokenUsage> }`
  - The message role is NOT a separate storage column — it lives inside the `message` JSON.

Storage is normalized `scenarios` + `rounds` tables (NOT a JSON blob). The domain `Scenario` is read/written whole; `scenario_repository` encapsulates the two-table mapping:

- `scenarios`: `id, chat_id, name, status, token_usage (nullable JSON), created_at, updated_at`
- `rounds`: `id, scenario_id, idx, message JSON, sources JSON, hidden (bool), token_usage (nullable JSON), created_at`

Persistence strategy:
- `insert_scenario` — new turn (scenario row + `rounds[0]`).
- `insert_round` — one round at a time during generation (incremental, O(N)).
- `update_scenario` — full rewrite (delete + re-insert rounds) only at finalization / continue-start.
- `votes` are keyed by `(chat_id, scenario_id)` (composite primary key).

The assistant `UIMessage.id` is the **first assistant round's** `message.id` (`rounds[1]`); `scenario_id` is carried separately in `metadata.scenarioId` (for voting) and never used as a message id.

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

The `z.strictObject` validation lives in the `ai` SDK package (`processUIMessageStream`,
used by `DefaultChatTransport`), not in `ui/src`. **All chunks must pass the
strict-object schema — extra fields break validation.**

`[DONE]` is optional. Stream ends when HTTP response body closes.

## SSE chunk registry

| Chunk | Key fields |
|-------|-----------|
| `start` | `messageId?`, `messageMetadata?` (carries `scenarioId`) |
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
| `data-*` | `id?`, `data` (e.g. `data-chat-title`, `data-round`) |
| `error` | `errorText` |
| `abort` | `reason?` |

## Steps (start-step / finish-step)

- Step = one LLM invocation. Multiple steps = tool-call loop.
- `start-step` → pushes `{ type: "step-start" }` into `message.parts[]` (invisible boundary).
- `finish-step` → resets `activeTextParts`/`activeReasoningParts`, does NOT produce a part.
- Simple text response: 1 step. Tool calls: 1 step per LLM iteration.
- During `convertToModelMessages`, `step-start` parts split the flat `parts[]` into separate
  assistant ModelMessage blocks per step.

## Rounds (data-round markers)

- A scenario has multiple assistant rounds; they are merged into ONE assistant `UIMessage`.
- Each round is marked with a `data-round` part `{ type: "data-round", data: { hidden: bool } }`
  inserted before the round's parts (streamed as a `data-round` SSE chunk, persisted the same way).
- `RoundDefinition.hidden` (default `false`) marks intermediate rounds hidden; the final answer
  round is visible. Persisted per-round in `rounds.hidden`.
- Frontend: `getRenderableParts(message, showHidden)` (utils/ai.ts) drops hidden rounds' parts for
  non-admins (`showHidden` = `isAdmin`) and strips structural markers (`step-start`, `source-url`);
  admins see everything. Sources are rendered separately via `getMessageSources(message)`, which
  reads the raw `parts[]` directly (no hidden gating), so they stay visible to everyone.
- Ordering invariant: `data-round` is emitted after the round's first `LlmCallStart`, so the `start`
  chunk (message id) always precedes it — otherwise the SDK would push a duplicate message.

## Tools

- Frontend does NOT auto-execute tools. `onToolCall` is undefined in Chat constructor.
- Set `providerExecuted: true` on all server-side tool chunks.
- Frontend tool rendering: `MessageContent.vue`, switch by `getToolName(part)`.
- Each tool component expects a specific output shape — mismatch causes
  `TypeError` during render, potentially freezing the UI.

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

- Home page (`index.vue`): `createChat()` → `POST /api/chats { firstMessage: prompt, model }`
  → `router.push('/chat/{id}')`
- Chat page (`[id].vue`):
  1. `GET /api/chats/{id}` → loads chat + messages (one user `UIMessage` + one assistant `UIMessage` per scenario)
  2. `new Chat({ id, messages, transport: DefaultChatTransport({ api: '/api/chats/{id}', headers, prepareSendMessagesRequest }) })`
     — `model` is injected per-request via `prepareSendMessagesRequest` → `buildSendMessageBody`
     (`utils/chatTransport.ts`, added only for new submissions), not a static transport `body`
  3. `onMounted`: if `isOwner` and `messages.length === 1`, calls `chat.regenerate()` (triggers continuation of the pending scenario)
- `chat.sendMessage({ text })`: appends user text, POSTs `{ trigger: "submit-message" }`
- `chat.sendMessage({ text, messageId })`: edit — backend deletes the scenario + reruns
- `chat.regenerate({ messageId })`: backend deletes the target scenario (+ later ones) and reruns `stream_new` with the stored user message (model taken from `scenario.name`, not from the frontend selector)
- Voting is keyed by `message.metadata.scenarioId`, not by message id.

### Frontend data schemas & validation

Use **Zod** (`zod` v4, already a dependency) for all data schemas, API response
validation, and tool input schemas — do not hand-roll type guards.

- Define the schema next to its domain type and derive the type from it with
  `z.infer`, so the type and the validator can never drift apart
  (e.g. `ui/src/types/branding.ts` exports both `brandingSchema` and `type Branding`).
- Parse untrusted data (API responses, SSE chunks) with `schema.parse(...)` or
  `schema.safeParse(...)`. Tool inputs use `inputSchema: z.object({...})`
  (see `ui/src/tools/*.ts`).
- Backend optional fields serialize to JSON `null` — mark them `.nullish()`
  (or `.nullable()`) rather than `.optional()`.

## Language

- All documentation, code comments, commit messages, and communication must be in English.
- `TODO.md` at the project root is the single source of truth for future work items.
  If the user asks to add a task for later, add it to `TODO.md`.
  When a task is completed, check `TODO.md` and remove the corresponding entry if present.
- `BRANDING.md` documents product branding configuration. Whenever branding
  behavior, the `CHIROO_BRANDING__*` env vars, or their defaults change, update
  `BRANDING.md` (and keep the README pointer in sync).

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

## Repository & client logging

All public functions in `server/src/infra/repositories/*.rs` and public methods in
`server/src/infra/clients/*.rs` get `#[instrument(level = "debug", skip(...))]`:

- Debug level keeps `info` logs clean in production; enable with `RUST_LOG=debug`
  (or a target filter) to see the call tree nested under the service spans.
- Log errors at the point they are caught — no entry logging, no exit `status`
  logging (that rule is for services only). Use `debug!(error = ..., status = "failed")`.
- Clients: inline `debug!` inside the `map_err` closure of each network call.
- Repositories: route DB errors through `map_infra_error` (`infra/errors.rs`),
  which logs and converts to `InfraError`.
- Skip `self` and `pool` always, plus any argument that is large, not `Debug`, or
  secret: byte blobs (`&[u8]`, `Vec<f32>`), content payloads (`NewMessageDb`,
  `DocumentChunk`, `CreateDocument`), `SecretBox`, tokens/codes, `metadata: &Value`.
- Do NOT instrument constructors (`new`) or private/helper methods.

Examples:
```rust
// client — network call error
.map_err(|e| {
    let msg = e.to_string();
    debug!(error = msg, status = "failed");
    AppError::QdrantError(msg)
})?;

// repository — DB error via shared helper
pub(crate) fn map_infra_error<E>(e: E) -> InfraError
where
    E: fmt::Display,
    InfraError: From<E>,
{
    debug!(error = e.to_string(), status = "failed");
    InfraError::from(e)
}
```

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
- Refresh flow: `verify_or_refresh` itself detects an expired access token (via the session's
  `expires_at`) and calls `refresh_session` — which looks up the session by `provider_sid` from
  the JWT's `sid` claim, refreshes via OIDC, upserts new tokens, and returns new `AuthUser` +
  new `access_token` (`Refreshed`). Middleware sets `Set-Cookie` with `path=/api` on the
  response. A *failed* refresh surfaces as `Expired` → middleware returns 401.
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
- Session TTL: session expiry comes from the refresh token's own `exp` claim when present;
  otherwise it falls back to `now + access_token_expires_in × session_ttl_multiplier`
  (multiplier configurable, default 10).
- Ownership checks: `chat_service::check_ownership(chat_id, user_id)` → lightweight
  `SELECT EXISTS(...)` via `chat_repository::is_owner`. Used in `send_message`, `get_votes`,
  `vote` handlers.

## AI service (`services/ai_service.rs`)

### Dependencies

`async-openai = { version = "0.40", features = ["chat-completion", "byot", "embedding"] }`

`AiService` owns `Client<OpenAIConfig>` and a model name. It orchestrates a whole `Scenario` (multiple rounds) per request; `LlmClient` (`infra/clients/llm_client.rs`) executes a single round. Each request spawns a tokio task that runs the round loop, accumulates events, persists rounds incrementally, and forwards SSE events. Handler only maps `StreamEvent → SseChunk → SSE`.

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

`run_round` uses a plain (unlabeled) `for step in 0..params.max_llm_calls` loop. Each iteration
calls `run_one_llm_call`, which streams one LLM call and returns a `StepResult`:

- `Stop { message, usage }` — final answer: append the message to the round, send
  `LlmCallFinish` and `return Ok(())` (the round is complete).
- `ToolCalls { message, completed_tools, usage }` — tools are executed at the end of the loop
  body, their results pushed to `messages`, and the loop naturally proceeds to the next step.
- `Err` — send an `Error` event and bail.

Loop exhaustion (call limit reached): if the round produced any output it is closed cleanly with
`LlmCallFinish(Stop)`; if it produced nothing, an `Error` event is sent.

### Scenario orchestration

Two public entry points (both return `(ScenarioId, MessageId, scenario_name, Stream<MessageStreamEvent>)`, where `scenario_name` is the resolved scenario/model id used for the `start` message metadata):

- `stream_new(chat_id, model, user_message, user_name)` — new turn: build a `Scenario` (status `Running`, `rounds=[user]`), persist, run all assistant rounds.
- `stream_continue(chat_id, scenario, user_name)` — resume an incomplete scenario from `next_round = scenario.rounds.len() - 1`.

`run_scenario` (private):
1. Resolves `ScenarioDefinition` by model name (`scenario_by_name`).
2. Loads all scenarios via `scenario_service.get_scenarios_for_chat(chat_id)` and builds
   the LLM history with the private pure function `build_history(scenarios, current_scenario_id)`:
   - the current scenario (continue only, passed as `Some(id)`) is included **in full** —
     its hidden rounds carry the tool outputs that later rounds of the same scenario depend on;
   - past scenarios contribute only their **visible** rounds (`hidden == false`), trimmed to
     text parts (reasoning / sources / tool calls dropped), capped to the most recent
     `PAST_HISTORY_LIMIT` (= 5, constant in `ai_service.rs`).
   For a new turn the scenarios are loaded **before** `insert_scenario` (the current
   question lives in the round prompt templates, not in history); for continuation
   they are loaded **after** `update_scenario` (already-run rounds must be visible).
3. Runs each round via `llm_client.execute_round`, collects the completed `Round { message, sources, hidden, usage }`, saves it incrementally with `insert_round`.
4. Sets `Completed` (or `Failed`) and does a final full `update_scenario`.

The `start` chunk's `messageId` is the **first assistant round's** `message.id` (pre-generated, forced onto round 0 via `message_id_override`).

Regeneration: `send_message` with `trigger: "regenerate-message"` + `messageId` → `find_scenario_by_message` (matches any round's message id) → delete that scenario + later ones → `stream_new` with the stored user message (`scenario.rounds[0]`) and model `scenario.name`.

### Control flow

```
AiService.stream_new / stream_continue → mpsc channel → tokio::spawn run_scenario:
  └─ per round: build OpenAI request with tools
  └─ llm_client.execute_round → run_round: for step in 0..max_llm_calls
        └─ send LlmCallStart (RoundMarker on step 0)
        └─ run_one_llm_call → create_stream_byot → iterate chunks
             ├─ reasoning_content → ReasoningDelta
             ├─ content → TextDelta
             ├─ tool_calls → ToolCallStart/Delta
             └─ StepResult:
                  ├─ ToolCalls → execute tools → next iteration
                  ├─ Stop → send LlmCallFinish → return
                  └─ Err → send Error → return
  └─ append Round { message, sources, hidden, usage } → insert_round (incremental)
```

## KB indexes & search filters

Knowledge bases have a nullable `indexes` JSON attribute declaring which top-level
document-metadata fields are indexable/filterable. Admin-set via the internal API or
`chiroo-cli kb create/update --indexes`. Shape: `{"<field>": {"type": "<type>", "description": "<text>"}}`;
allowed types: `keyword, integer, float, bool, datetime, uuid`. Only top-level
document-metadata keys are indexable (field names may not contain `.`). Validated at
write time — 400 on bad input.

Domain types:
- `domain/models/kb_index.rs` — `IndexType`, `KbIndex`, `KbIndexes::parse`.
- `domain/models/search_filter.rs` — `SearchFilter`, `from_llm_value` (parses the tool
  `filters` argument), `validate` (against a `KbIndexes`), `scope`.

Qdrant: the chunk payload stores the document `metadata` as a structured object under
the payload key `metadata` (NOT a JSON string); the indexable path is always
`metadata.<field>`. Field indexes are created/removed/retyped by
`QdrantClient::sync_field_indexes` on KB create/update — Qdrant backfills new indexes
over existing points, so changing `indexes` needs no document reindex. NOTE: after the
string→object payload format change, existing collections need a one-time
`chiroo-cli kb reindex --mode full` for filters to work on old chunks.

The internal update endpoint (`PUT /internal/api/knowledge-bases/{id}`) is a full
replace: omitting `indexes` in the request resets it to null, which also removes the
KB's Qdrant `metadata.*` field indexes on the following sync. `chiroo-cli kb update`
pre-loads the current KB to preserve omitted values; to clear indexes via the CLI pass
`--indexes '{}'` (an explicit `null` to the API clears them directly).

Search: two tools — `search_in_all_kbs` (broad search across all KBs, or a subset via
`kb_ids`; no filters) and `search_in_kb` (single KB by id; `filters` validated strictly
against that KB's indexes). Filter format ("form A"):
object, conditions combined with AND, no nesting; a bare scalar value = equality; a range
= object with any of `eq/gt/gte/lt/lte`; ranges only for `integer`/`float`/`datetime`;
datetime values are RFC 3339 strings. Datetimes have second granularity (Qdrant datetime
indexes truncate sub-seconds). Filters are applied to both hybrid-search prefetches
(dense + BM25) via `search_hybrid(..., filter)`.

The system prompt (`prompts/global_system.md`) lists each KB's "Filterable metadata
fields".


## OpenAI-compatible API (`/api/openai/v1`)

A stateless, OpenAI-client-compatible surface for scripts and integrations.
Disabled by default; mounted only when `openai_api.enabled` is true
(`CHIROO_OPENAI_API__ENABLED`), otherwise the routes 404.

- Auth: OIDC access token via `Authorization: Bearer` (dedicated middleware
  `middleware/openai_auth.rs`, NOT the cookie/CSRF middleware). No `sk-`/static
  keys. The token is verified against the JWKS (signature, issuer, `exp`) via
  `OidcClient`, and the user is resolved strictly by `users.sso_id = sub`
  (`AuthService::verify_api_token` → `ApiPrincipal` in request extensions).
  No session lookup, no refresh, no cookies. Unknown `sub` → 401.
- Wire format: OpenAI shapes, snake_case (no camelCase renames). Errors use the
  OpenAI envelope `{"error": {message, type, param, code}}`; 401s carry
  `WWW-Authenticate: Bearer`.
- `POST /chat/completions` is stateless: the caller sends the full
  `messages[]`; Chiroo creates a normal chat (`ChatService::create_chat_shell`,
  no auto scenario) and runs one scenario via `AiService::complete` (synchronous
  drain of the normal scenario pipeline; `system`/`developer` messages are
  appended to the global prompt via `TemplateContext.client_system`, never
  replacing it). The provided history is used for the LLM but not re-persisted.
- Rejected explicitly: `stream: true`, client tools, `response_format`,
  `n != 1`, `logprobs`; sampling fields are ignored. Model ids are native
  (`simple`, `deep_research`) — no aliases.
- Response extensions: `sources` (deduped across all rounds, incl. hidden) and
  `chiroo` (`chat_id`, `scenario_id`).
- Service users are created/deleted explicitly (internal API `POST/DELETE
  /internal/api/users`, CLI `chiroo-cli user create/delete`); deletion keeps
  chats by default, `?purgeChats=true` / `--purge-chats` deletes them in one
  transaction.
