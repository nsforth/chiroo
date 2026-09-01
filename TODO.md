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

`AuthService::verify_or_refresh` — no structured logs for auth errors (`Invalid`, `Expired`, DB errors). Only ad-hoc `tracing::info` / `tracing::debug` calls.

### Investigate `trigger` field in SendMessageRequest

`MessageTrigger` — `trigger` field comes from Vercel AI SDK. Figure out what it is and how to handle it.

### Seamless resume after a dropped stream

The backend can continue an incomplete scenario (`stream_continue` resumes from `next_round`),
but the frontend has no way to resume a partially-completed scenario after a disconnect:
`chat.regenerate()` slices off the partial assistant message, so re-streaming only the remaining
rounds would drop the already-rendered rounds. Needs a design: re-stream the whole scenario from
scratch on resume, or a custom continuation that appends to the existing message
(`resumeStream`/reconnect), plus detecting an incomplete scenario (`status != Completed`) on load.

### Mark the final-answer round with a backend flag (`isFinalAnswer`)

**Diagnosis (sources cutoff after streaming):**

The sources list (`ChatMessageSources`) is gated by a `streaming` prop =
`chat.status === 'streaming' && message.id === chat.messages.at(-1)?.id`
(`[id].vue` → `MessageContent.vue`), so it only appears once the whole SSE
stream ends (`status` flips to `'ready'` in `chat.ts`). At that moment:

- The list is inserted (+~50–60px) at the bottom of the last message, but
  `ChatMessages` auto-scroll only runs while `status === 'streaming'`
  (`ChatMessages.vue:75–87, 180–183`) → the new sources land below the visible
  fold and are never scrolled into view (chat "doesn't finish scrolling",
  sources partially cut off at the bottom).
- The status bar disappears (sticky bottom block shrinks, `spacing-offset`
  `188 → 160`) while `--last-message-height` (computed once at submit-time by
  `ChatMessages`) is not recomputed → stale sizing for short last messages.

Root cause: "the stream is over" is the wrong signal. What's needed is "the
final answer part has started".

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

---

## Bugs

## Tech Debt

### LLM round: validate non-empty system prompt

`llm_client.rs` `run_round` — silently skips the system message when `system_prompt` is empty. Validate a non-empty system prompt once at scenario/round construction (fail fast) instead of the per-round runtime empty-check.

### Repositories: domain models on input/output

All repository functions should accept and return domain models instead of `*Db` raw Diesel structs. `*Db` structs should only be used internally within the repository module. Currently most repositories expose `*Db` types publicly and require manual `.into()` / conversion at the call site.

Example: `user_repository::upsert` takes individual fields instead of a `User` domain model.

Affected:
- `chat_repository.rs` — `NewChatDb`, `ChatDb` exposed
- `session_repository.rs` — `SessionDb` exposed
- `vote_repository.rs` — `VoteDb`, `NewVoteDb` exposed
- `user_repository.rs` — `UserDb` exposed, `upsert` takes individual fields

### Replace `serde_json::Value` with `UIMessage`

`stream.rs` — `SendMessageRequest.messages: Vec<serde_json::Value>` instead of typed `Vec<UIMessage>`.

### Stream handler: struct + state machine

`domain_to_sse_chunks` — uses a mutable `started: &mut bool` flag. Replace with a proper state machine.

### Extract UIMessage converter from handler

`votes.rs` — `VoteDb → VoteResponse` conversion done inline in `get_votes`. Move to a dedicated converter / `From` impl.

### ChatService: full review

`ChatService` — review logic and error handling.

### OIDC metadata cache: refresh entire metadata

`fetch_jwk_set` — when JWKS is refreshed, the whole discovery metadata (issuer, endpoints, JWK cache) must be refreshed too, otherwise keys and metadata become desynchronized.

### Stream errors: make them specific

`MessageStreamEvent::Error` — carries no reason. Replace with a structured variant containing an error text or code.

### Search round: kb_id hallucination has no self-correct path

`search_knowledge_base` rejects bad `kb_ids` with a tool error ("Invalid knowledge base id: ..." /
"Unknown knowledge base id: ..."), but `simple/search` and `deep_research/search` rounds have
`max_llm_calls: 1`, so a hallucinated id ends the round with a tool error: extract runs without
document_ids and the final answer degrades to "not found" even when the data exists.
Only `deep_research/explore` (max_llm_calls: 10) can self-correct today.

Planned fix (apply if real failures are observed):
- include the list of valid KB ids in the tool error message
- raise search-round `max_llm_calls` to 2

### Markdown render: defensive fallback on parse failure

Comark parses LLM/assistant and tool (search/extract) markdown without a `.catch` in its async `setup`, so a parser crash blanks the whole part (no console error). Add a safe fallback (pre-parse in try/catch → render plain escaped text on failure) in `ChatComark.vue` and the tool content components (`SearchKnowledgeBase.vue`, `ExtractDocument.vue`). Deferred after the comark 0.3→0.6 upgrade, which fixed the known nested-bracket link crash.
