# Chiroo

AI chat webapp focused on agentic RAG that is really works.

## Architecture

```
chiroo/
├── ui/              # Frontend (Vue 3 + Vite)
│   └── src/
│       ├── api/          # Fetch wrapper (apiFetch)
│       ├── components/   # Vue components
│       ├── composables/  # Reactive state & actions
│       ├── layouts/      # Page layouts
│       ├── locales/      # i18n translations (en, ru)
│       ├── pages/        # Vue Router pages
│       ├── plugins/      # Nuxt-style plugins
│       ├── tools/        # AI SDK tool implementations (chart)
│       ├── types/        # TypeScript types
│       └── utils/        # Shared helpers
└── server/          # Backend (Rust + Axum)
    ├── src/         # Source code (DDD layout)
    │   ├── bin/     # chiroo-cli (CLI tool)
    │   ├── domain/  # Pure domain models
    │   ├── handlers/# HTTP layer
    │   ├── services/# Business logic
    │   ├── infra/   # Diesel schema, repositories
    │   ├── middleware/ # Auth, CSRF & admin middleware
    │   └── utils/   # Shared utilities
    └── migrations/  # Diesel ORM migrations
```

## Tech Stack

### Frontend (`ui/`)

- **Framework:** Vue 3, Vite
- **UI:** Nuxt UI, Tailwind CSS
- **AI SDK:** Vercel AI SDK (streaming chat via SSE)
- **Routing:** Vue Router
- **i18n:** vue-i18n (EN, RU)

### Backend (`server/`)

- **Framework:** Axum (Tokio)
- **ORM:** Diesel
- **Database:** PostgreSQL
- **Vector store:** Qdrant (embeddings + BM-25)
- **Auth:** OIDC / Keycloak (confidential client, auth code flow)
- **AI Client:** async-openai (streaming + tool calls)
- **Architecture:** Domain-Driven Design

## Quick Start

### Prerequisites

- Node.js 20.19+ (or 22.12+) and pnpm
- Rust (latest stable)
- PostgreSQL 15+
- Qdrant (vector store for embeddings/search)
- Keycloak (or any OIDC-compatible provider)
- External openai api compatible LLM, embeddings and reranker

### Setup

```bash
# Frontend
cd ui
pnpm install

# Backend
cd server
cp .env.example .env # Write your own secrets, urls, etc...
cargo build
```

### Run

```bash
# Frontend dev server (http://localhost:5173)
cd ui && pnpm dev

# Backend dev server (http://127.0.0.1:3000)
cd server && cargo run            # built-in defaults + server/.env
# or override with a config file (see Configuration below):
cd server && cargo run -- -c config/default.yaml
```

The Vite dev server proxies `/api/*` requests to the backend automatically.

## Commands

| Command | Description |
|---|---|
| `pnpm dev` (ui) | Start frontend dev server |
| `pnpm build` (ui) | Production build |
| `pnpm typecheck` (ui) | TypeScript type check |
| `pnpm lint` (ui) | ESLint |
| `cargo run` (server) | Start backend in dev mode |
| `cargo check` (server) | Compile-check without producing a binary |
| `cargo build --release` (server) | Production build |

## Configuration

### Backend (`server/.env`)

Configuration is resolved from three layers, lowest to highest precedence:

1. **Built-in defaults** — a YAML file compiled into the binary
   (`server/config/default.yaml`).
2. **A config file** passed with `-c` / `--config` (format detected from the
   extension: `yaml`, `yml`, `json`, `toml`, `ini`, ...). Copy
   `server/config/default.yaml` as a starting point.
3. **Environment variables** — `CHIROO_*` vars, including anything loaded from
   `server/.env`.

The env var name is `CHIROO_<SECTION>__<FIELD>` (`__` separates the section from
the field, a single `_` stays inside a field), e.g. `CHIROO_OPENAI__BASE_URL`.

| Variable | Required | Default | Description |
|---|---|---|---|
| `CHIROO_SERVER__HOST` | | `127.0.0.1` | Server bind address |
| `CHIROO_SERVER__PORT` | | `3000` | Server port |
| `CHIROO_INTERNAL_API__HOST` | | `127.0.0.1` | Internal API bind address |
| `CHIROO_INTERNAL_API__PORT` | | `3001` | Internal API port |
| `CHIROO_DB__URL` | ✅ | — | PostgreSQL connection string |
| `CHIROO_DB__ENCRYPTION_KEY` | ✅ | — | 32-byte key for field-level encryption (a 32-char string, e.g. `openssl rand -hex 16`) |
| `CHIROO_OPENAI__BASE_URL` | ✅ | — | OpenAI-compatible API endpoint |
| `CHIROO_OPENAI__API_KEY` | ✅ | — | API key |
| `CHIROO_OPENAI__MODEL` | ✅ | — | Model name (e.g. `gpt-4o`) |
| `CHIROO_OPENAI__INCLUDE_USAGE` | | `true` | Request token usage in streaming completion responses |
| `CHIROO_OPENAI_API__ENABLED` | | `true` | Enable the OpenAI-compatible API at `/api/openai/v1` (see below) |
| `CHIROO_EMBEDDING__BASE_URL` | ✅ | — | Embeddings API endpoint |
| `CHIROO_EMBEDDING__API_KEY` | ✅ | — | Embeddings API key |
| `CHIROO_EMBEDDING__MODEL` | ✅ | — | Embedding model name |
| `CHIROO_EMBEDDING__DIMENSIONS` | ✅ | — | Embedding vector dimensions |
| `CHIROO_RERANK__BASE_URL` | ✅ | — | Reranking API endpoint |
| `CHIROO_RERANK__API_KEY` | ✅ | — | Reranking API key |
| `CHIROO_RERANK__MODEL` | ✅ | — | Reranking model name |
| `CHIROO_QDRANT__URL` | | `http://localhost:6334` | Qdrant server URL |
| `CHIROO_QDRANT__API_KEY` | | — | Qdrant API key (if auth is enabled) |
| `CHIROO_QDRANT__COLLECTION_PREFIX` | | `chiroo` | Prefix for Qdrant collection names |
| `CHIROO_QDRANT__TIMEOUT` | | `30s` | Qdrant request timeout (duration string, e.g. `30s`, `5m`) |
| `CHIROO_CHUNKING__MAX_CHUNK_SIZE` | | `512` | Max document chunk size |
| `CHIROO_SEARCH__LIMIT` | | `10` | Max search results returned |
| `CHIROO_SEARCH__SCORE_THRESHOLD` | | `0.5` | Minimum relevance score |
| `CHIROO_SEARCH__CANDIDATE_LIMIT` | | `30` | Max candidates before reranking |
| `CHIROO_OIDC__APP_BASE_URL` | ✅ | — | Public URL of the app (used for OIDC redirect) |
| `CHIROO_OIDC__ISSUER_URL` | ✅ | — | OIDC issuer (Keycloak realm URL) |
| `CHIROO_OIDC__CLIENT_ID` | ✅ | — | OIDC client ID |
| `CHIROO_OIDC__CLIENT_SECRET` | ✅ | — | OIDC client secret |
| `CHIROO_OIDC__STATE_SECRET` | ✅ | — | HMAC key for state/CSRF tokens (min 32 chars) |
| `CHIROO_OIDC__CLEANUP_INTERVAL` | | `600s` | Expired session cleanup interval (duration string) |
| `CHIROO_OIDC__SESSION_TTL_MULTIPLIER` | | `10` | Session TTL = access_token TTL × multiplier |
| `CHIROO_OIDC__ACCESS_TOKEN_EXPIRES` | | `300s` | Fallback access_token TTL if not in OIDC response (duration string) |

Product branding is configured through the `branding` section
(`CHIROO_BRANDING__*` variables) — see [BRANDING.md](BRANDING.md).

### Frontend (`ui/`)

The frontend requires no environment variables. All API communication goes through the Vite proxy to the backend.

Product branding (name, label, tagline, colors, logos, fonts) is fetched from the backend (`GET /api/branding`) before the app is mounted; the frontend keeps no branding assets or defaults of its own.

## OpenAI-compatible API

Chiroo exposes a stateless OpenAI-compatible endpoint for scripts and
integrations, so existing OpenAI SDKs and clients can call it without a custom
protocol layer. It is enabled by default; disable it with
`CHIROO_OPENAI_API__ENABLED=false` (or `openai_api.enabled: false` in a config
file). When disabled the routes are not mounted and return 404.

- **Base URL:** `/api/openai/v1`
  - `GET /api/openai/v1/models` — lists the available models
  - `GET /api/openai/v1/models/{model}` — fetch one model
  - `POST /api/openai/v1/chat/completions` — run a scenario
- **Models:** `simple` and `deep_research` (Chiroo's own scenario ids — no
  `gpt-*` aliases, unknown models are rejected with 404).
- **Stateless:** the caller sends the full `messages[]` history on every call;
  Chiroo creates a new chat for the turn, runs one scenario through its normal
  pipeline (search, hidden tool rounds, sources), persists it like a regular
  UI chat, and returns a single non-streaming response.

Authentication is an OIDC access token in the `Authorization` header — there
are no `sk-` API keys. The token is verified against the OIDC provider's JWKS
and the user is resolved strictly by `users.sso_id = <sub>`; a `sub` with no
user row is rejected with 401.

### Service users

Service accounts never log in through the UI, so create them explicitly (the
`sub` is the user's OIDC identity — the same `sso_id` Keycloak assigns at
login, visible in Keycloak under the user's identity/credentials):

```bash
# via CLI (talks to the internal API)
chiroo-cli user create \
  --sso-id <oidc-sub> \
  --sso-provider oidc \
  --username service-user \
  [--name "Service User"] [--email service@example.com]

# or via the internal API
curl -X POST http://127.0.0.1:3001/internal/api/users \
  -H 'Content-Type: application/json' \
  -d '{"ssoId": "<oidc-sub>", "ssoProvider": "oidc", "username": "service-user"}'
```

Users are always created as non-admins. Admin access is granted (or revoked)
only through the separate admin call:

```bash
chiroo-cli user change --id <uuid> --admin yes
```

Delete a user (its API access is revoked immediately, since API auth resolves
strictly by `sso_id`):

```bash
chiroo-cli user delete --id <uuid>          # deletes the user; keeps their chats
chiroo-cli user delete --id <uuid> --purge-chats   # also deletes their chats
```

Deleting a user **keeps their chats by default** (valuable data);
`--purge-chats` is the explicit destructive path.

### Example

```bash
curl http://127.0.0.1:3000/api/openai/v1/chat/completions \
  -H "Authorization: Bearer $OIDC_ACCESS_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{
    "model": "simple",
    "messages": [
      {"role": "user", "content": "What does the onboarding policy say about remote work?"}
    ]
  }'
```

The response is a standard OpenAI `chat.completion` object with two
documented extensions:

- `sources` — the documents the answer is grounded in (collected from all
  rounds, including hidden search rounds), always present as an array.
- `chiroo` — `{ "chat_id", "scenario_id" }` for inspecting the persisted
  chat/scenario.

Caller `system`/`developer` messages are appended to Chiroo's global system
prompt (they do not replace it).

### Limitations

- **Non-streaming only:** `stream: true` is rejected; so are client-side
  tools (`tools`, `tool_choice`, `functions`) and `response_format`.
  Harmless sampling fields (`temperature`, `top_p`, `max_tokens`, `stop`,
  `seed`, `user`) are accepted and ignored.
- **`deep_research` is long-running** (multiple LLM + search steps). There is
  no server-side completion timeout — your HTTP client owns the timeout policy.

## License

MIT
