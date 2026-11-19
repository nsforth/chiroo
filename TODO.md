# TODO

## Security

### Logout / session invalidation

No logout endpoint exists. `clearSession()` in `useUserSession.ts` only nulls a local reactive ref.

Needs:
- Backend `POST /api/auth/logout` — delete session from DB, clear cookies
- Keycloak RP-Initiated Logout to end the session on the provider side

---

## Features

### Make state token TTL configurable

`encode_state_token` — state token `exp` hardcoded to 300 seconds. Move to `OidcConfig`.

### Retry logic for refresh_token

`OidcClient::refresh_token` — refresh fails immediately on transient OIDC errors. Add retry with exponential backoff.

### Structured auth error logging

`AuthService::verify_or_refresh` — the main path logs `info!(auth_result = %result, ...)`, but the
`Err(_e) => AuthResult::Expired` branch swallows the refresh failure reason. Log the error
when a token refresh fails.

### Finalize the previous scenario when a new message is sent

Sending a new message while the previous scenario is still incomplete (`Running` /
`Failed`) has no defined behavior. Needs a design: reject the new message, wait,
or finalize/cancel the previous scenario before starting the new one.

### Seamless resume after a dropped stream

Resume already works for the first turn: the frontend's no-op `chat.regenerate()` maps to the
`continue` trigger (`chatTransport.ts`), the backend `StreamRequest::Continue` finds the
incomplete scenario and `stream_continue` re-streams from `next_round`, and the `start` chunk
carries the persisted first assistant round's message id, so the SDK appends to the existing
message instead of dropping the partial one.

Remaining: auto-resume on page load fires only when `messages.length === 1` (`[id].vue`
`onMounted`). For chats with several messages whose last scenario is incomplete (`Running` /
`Failed`) there is no resume on load. Needs a design: detect an incomplete scenario
(`status != Completed`) on load and trigger `continue` for such chats.

### Migrate the chat transport to WebSocket + pub/sub

Long-running scenarios (e.g. `deep_research`) can run for minutes; losing the stream on a network
blip wastes tokens. Current HTTP SSE transport can't resume mid-stream or push server-side events.

Planned:
- Backend: WebSocket endpoint (`axum::ws`) + a pub/sub layer (e.g. STOMP/broker) so the server can
  deliver only the chunks a client hasn't received yet after a reconnect.
- Frontend: custom `ChatTransport` (SDK supports pluggable transports via
  `sendMessages(): Promise<ReadableStream<UIMessageChunk>>`) over WebSocket; enables several
  concurrently active chats and server-initiated updates.
- Post-scenario-downtime resume is already covered by the `continue` trigger (see the
  "Seamless resume after a dropped stream" item); this item is about mid-stream resume after a
  network blip, server-initiated pushes, and several concurrently active chats.

### Mark the final-answer round with a backend flag (`isFinalAnswer`)

**Diagnosis (sources cutoff after streaming):**

The sources list (`ChatMessageSources`) is gated in `MessageContent.vue` by
`showSources = role === 'assistant' && sources.length > 0 && (!streaming || hasTextPart)`,
so it appears either as soon as the message's first text part arrives or when the whole SSE
stream ends. Both signals are frontend heuristics: sources can appear at the bottom of a long
message after streaming has already stopped, where `ChatMessages` auto-scroll (which only runs
while `status === 'streaming'`) will not bring them into view, and the sticky `spacing-offset` /
`--last-message-height` sizing goes stale when the status bar disappears.

Root cause: "the stream is over" / "text started" is the wrong signal. What's needed is
"the final answer round has started".

**Solution: mark the final-answer round from the backend.**

The backend already knows the answer round: the last round of the scenario
(`round_index == rounds.len() - 1`, the no-tools `answer` round in
`simple`/`deep_research`). Show sources exactly when that round starts — still
within `status === 'streaming'`, so auto-scroll keeps the growing content
(incl. sources) in view; the manual scroll-on-`ready` workaround is not needed.

Flag semantics: message-level metadata flag. It will usually land on the single
last part of the final answer, but relying on "the flag is present on at least
one part" is fine (frontend should not assume which part carries it).

Backend:
- `run_scenario` (`ai_service.rs`): at the start of the final round emit a new
  `MessageStreamEvent` (e.g. `FinalAnswerStart`).
- `domain_to_sse_chunks` (`stream.rs`): map it to the SSE `message-metadata`
  chunk —
  `{"type":"message-metadata","messageMetadata":{"scenarioId":"…","isFinalAnswer":true}}`.
  The SDK merges it into `message.metadata` mid-stream (`process-ui-message-stream.ts:795`,
  `mergeObjects`), preserving `scenarioId`; no `z.strictObject` breakage.
- Persisted messages (`ui_messages.rs` `scenario_to_ui_messages`): set
  `isFinalAnswer: true` in the assistant message metadata when the scenario is
  `Completed` (last round is the answer round) so the flag survives reloads.

Frontend:
- `MessageContent.vue`: show `ChatMessageSources` when
  `sources.length && message.metadata.isFinalAnswer`; drop the `streaming` prop
  gate (and the earlier part-state `isStreaming` heuristic).
- The flag generalizes the non-admin rendering (which today hides intermediate
  tool/reasoning parts via `showDetails`/part types) into an explicit backend
  signal; can later drive collapsing intermediate steps for all users.

**Alternative: push sources in one shot on the final round instead of continuously.**

Instead of streaming `source-url` chunks as they are found (which causes the
list to grow and re-render mid-stream), collect sources during the search/extract
rounds and emit them all at once at the start of the final answer round.

Rationale:
- The model can be asked, via a dedicated tool (e.g. `report_sources`), to list
  the sources it actually used in the answer — not every source found. More
  useful output, and the bug resolves itself: the list appears only when the
  final answer round starts (still within `status === 'streaming'`, so
  auto-scroll keeps it in view), without needing the `isFinalAnswer` flag.
- Implementation sketch: add a `report_sources` tool to the answer round of
  `simple`/`deep_research`; the backend buffers `round_sources` and on
  `report_sources` (or at `LlmCallFinish` of the last round) emits the
  `source-url` chunks in one batch. Persisted sources still come from
  `scenario.rounds[..].sources` as today.

### Validate document metadata values against declared `indexes`

`knowledge_service.rs` `create_document` / `update_document` don't check that metadata
values for fields declared in the KB's `indexes` match the declared type. A wrong-typed
value (e.g. a number where `keyword` is declared) is silently skipped by the Qdrant field
index, so filters over that field return fewer/empty results with no error. Add fail-fast
validation at document write: for each declared index field present in the document
metadata, verify the value's type matches the index type and reject the request with a
clear error naming the field and expected type.

---

## Bugs

## Tech Debt

### LLM round: validate non-empty system prompt

`llm_client.rs` `run_round` — silently skips the system message when `system_prompt` is empty. Validate a non-empty system prompt once at scenario/round construction (fail fast) instead of the per-round runtime empty-check.

### Repositories: domain models on input/output

All repository functions should accept and return domain models instead of `*Db` raw Diesel structs. `*Db` structs should only be used internally within the repository module. Currently most repositories expose `*Db` types publicly and require manual `.into()` / conversion at the call site.

Example: `user_repository::upsert` takes individual fields instead of a `User` domain model.

`session_repository` is already refactored (`SessionDb` private, public API on the `Session` domain model) — use it as the pattern.

Remaining:
- `chat_repository.rs` — `NewChatDb`, `ChatDb` exposed
- `vote_repository.rs` — `VoteDb`, `NewVoteDb` exposed
- `user_repository.rs` — `UserDb` exposed, `upsert` takes individual fields

### Stream handler: struct + state machine

`domain_to_sse_chunks` — uses a mutable `started: &mut bool` flag. Replace with a proper state machine.

### ChatService: full review

`ChatService` — review logic and error handling.

### OIDC metadata cache: refresh entire metadata

Discovery metadata (issuer, endpoints, `jwks_uri`) is fetched once in `OidcClient::new()` and never refreshed (the client lives in an `OnceCell` for the process lifetime). Only the JWKS has a cache: `#[cached(ttl = 3600)]` on `fetch_jwk_set`. On a JWKS refresh the whole discovery metadata must be refreshed too, otherwise keys and metadata become desynchronized (rotated JWKs vs. stale issuer/endpoints).

### Search round: kb_id hallucination has no self-correct path

`search_in_all_kbs` / `search_in_kb` reject bad `kb_ids` / `kb_id` with a tool error ("Invalid
knowledge base id: ..." / "Unknown knowledge base id: ..." / "knowledge base not found: ..."),
but `simple/search` and `deep_research/search` rounds have
`max_llm_calls: 1`, so a hallucinated id ends the round with a tool error: extract runs without
document_ids and the final answer degrades to "not found" even when the data exists.
Only `deep_research/explore` (max_llm_calls: 10) can self-correct today.

Planned fix (apply if real failures are observed):
- include the list of valid KB ids in the tool error message
- raise search-round `max_llm_calls` to 2

### Tool-call retry budget (`max_tool_retries`)

Tool errors (bad filter, hallucinated `kb_id`, wrong-typed value) currently consume
`max_llm_calls`; a failed tool call in a `max_llm_calls: 1` round ends the round without
self-correction. Related to "Search round: kb_id hallucination has no self-correct path".
Introduce a separate `max_tool_retries` budget (config, `OpenAiConfig`) so error-driven
retries don't consume the successful-call budget: `max_llm_calls` counts successful calls
(Stop or all tools executed without error), tool errors consume `max_tool_retries` instead.
Deferred — observe how reliably the model drives the tools; implement only if tool errors
recur regularly.

### LLM history: push the scenario limit to the DB

`build_history` (`ai_service.rs`) loads **all** scenarios of the chat via
`get_scenarios_for_chat` and only then trims past scenarios to the most recent
`PAST_HISTORY_LIMIT` in memory. For long conversations this reads the entire dialog
(every round + JSON payload) to throw most of it away. Add a repository method that lets
the DB select only the last N scenarios (with their rounds) and fetch just those.

### Qdrant datetime granularity

Qdrant datetime indexes have second granularity: sub-seconds in document metadata values and
filter values are silently truncated (an `eq` on a datetime can match all values within the
same second). Needs a decision: truncate at document save (only fields declared `datetime` in
`indexes`), truncate/validate filter values, and keep DB metadata ↔ Qdrant payload consistent
on reindex — currently they can drift.

---

## Testing

Note: there is no `server/tests/` directory — all tests are inline `#[cfg(test)]` modules in `src/`. Service-level integration tests against Postgres exist for `vote_service`, `scenario_service`, `search_service`, and `knowledge_service` (all `#[ignore] = "requires Postgres…"`); they do not cover the items below.

### AiService: end-to-end scenario orchestration test

No live test covers `AiService` orchestration (`stream_new` / `stream_continue`): multi-round
execution, tool-call loop (multiple steps), incremental `insert_round`, final status, and SSE
invariants (`start.messageId` = first assistant round id, `start` before `data-round`,
`metadata.scenarioId`). Needs LLM + embedding + Qdrant + Postgres; `#[ignore]` like the other
live tests.

### Auth: live tests against Keycloak

`OidcClient` (metadata, JWKS, code/token exchange) and `AuthService` (`verify_token`,
`verify_or_refresh` Valid/Refreshed/Expired/Invalid, refresh flow) have no integration tests.
Needs a local Keycloak instance; `#[ignore]`.

### HTTP-level e2e tests

Zero coverage at the handler layer. Candidates: POST `/api/chats` → SSE chunk contract
(ordering + strictObject compatibility with the frontend schema), auth login → callback →
cookies, middleware expired-refresh + CSRF failures, internal API 400s on bad `indexes` /
document metadata.
