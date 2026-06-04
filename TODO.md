# TODO

## Security

### Logout / session invalidation

No logout endpoint exists. `clearSession()` in `useUserSession.ts` only nulls a local reactive ref (`// TODO: implement logout with Keycloak end session`).

Needs:
- Backend `POST /api/auth/logout` — delete session from DB, clear cookies
- Keycloak RP-Initiated Logout to end the session on the provider side

### Encrypt tokens at rest

`access_token` and `refresh_token` stored as plain `TEXT` in the `sessions` table (see `session_repository.rs:50`: `// TODO Encryption`).

If the DB is compromised, all active tokens (access + refresh) are exposed.

Needs: encrypt both fields on write, decrypt on read. Encryption key from config (e.g. `STATE_SECRET` or a dedicated `TOKEN_ENCRYPTION_KEY`).

---

## Features

### Make state token TTL configurable

`oidc_client.rs:38` — state token `exp` hardcoded to 300 seconds. Move to `OidcConfig`.

### Retry logic for refresh_token

`oidc_client.rs:290` — refresh fails immediately on transient OIDC errors. Add retry with exponential backoff.

### Structured auth error logging

`auth_service.rs:61-62` — no structured logs for auth errors (`Invalid`, `Expired`, DB errors). Only ad-hoc `tracing::info` / `tracing::debug` calls.

### Investigate `trigger` field in SendMessageRequest

`stream.rs:99` — `trigger` field comes from Vercel AI SDK. Figure out what it is and how to handle it.

### Investigate `convert_parts_to_ui`

`chats.rs:221` — `// TODO: Investigate what is this AI generated shit`. The function is written in an ad-hoc manner with fallbacks, needs a proper architectural rewrite.

---

## Tech Debt

### Repositories: domain models on input/output

All repository functions should accept and return domain models instead of `*Db` raw Diesel structs. `*Db` structs should only be used internally within the repository module. Currently most repositories expose `*Db` types publicly and require manual `.into()` / conversion at the call site.

Example: `user_repository::upsert` takes individual fields instead of a `User` domain model (`// TODO: Change to save and User model from caller`).

Affected:
- `chat_repository.rs` — `NewChatDb`, `ChatDb` exposed
- `session_repository.rs` — `SessionDb` exposed
- `message_repository.rs` — `MessageDb`, `NewMessageDb` exposed
- `vote_repository.rs` — `VoteDb`, `NewVoteDb` exposed
- `user_repository.rs` — `UserDb` exposed, `upsert` takes individual fields

### Replace `serde_json::Value` with `UIMessage`

`stream.rs:96` — `messages: Vec<serde_json::Value>` instead of typed `Vec<UIMessage>`.

### Stream handler: struct + state machine

`stream.rs:156` — `domain_to_sse_chunks` uses a mutable `started: &mut bool` flag. Replace with a proper state machine.

### Extract UIMessage converter from handler

`chats.rs:120, 143` — `MessageDb → UIMessage` conversion done inline in `get_chat`. Move to a dedicated converter / `From` impl.

- `votes.rs:46` — same for `VoteDb → VoteResponse`

### ChatService / MessageService: full review

`chat_service.rs:15`, `message_service.rs:14` — `// TODO: Full review, has some shit`. Review logic and error handling.

### OIDC metadata cache: refresh entire metadata

`oidc_client.rs:60` — when JWKS is refreshed, the whole discovery metadata (issuer, endpoints, JWK cache) must be refreshed too, otherwise keys and metadata become desynchronized.

### Stream errors: make them specific

`stream_event.rs:66` — `LlmCallFinish::Error` carries no reason. Replace with a structured variant containing an error text or code.
