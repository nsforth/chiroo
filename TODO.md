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

### Markdown render: defensive fallback on parse failure

Comark parses LLM/assistant and tool (search/extract) markdown without a `.catch` in its async `setup`, so a parser crash blanks the whole part (no console error). Add a safe fallback (pre-parse in try/catch → render plain escaped text on failure) in `ChatComark.vue` and the tool content components (`SearchKnowledgeBase.vue`, `ExtractDocument.vue`). Deferred after the comark 0.3→0.6 upgrade, which fixed the known nested-bracket link crash.
