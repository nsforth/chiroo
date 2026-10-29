# Chiroo

AI chatbot with a full-stack architecture: Vue 3 frontend + Rust backend.

## Architecture

```
chiroo/
├── ui/              # Frontend (Vue 3 + Vite)
│   └── src/
│       ├── components/   # Vue components
│       ├── composables/  # Reactive state & actions
│       ├── pages/        # Vue Router pages
│       ├── tools/        # AI SDK tool implementations (weather, chart)
│       └── types/        # TypeScript types
└── server/          # Backend (Rust + Axum)
    ├── src/         # Source code (DDD layout)
    │   ├── domain/  # Pure domain models
    │   ├── handlers/# HTTP layer
    │   ├── services/# Business logic
    │   ├── infra/   # Diesel schema, repositories
    │   ├── middleware/ # Auth & CSRF middleware
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
- **Vector store:** Qdrant (embeddings + reranking)
- **Auth:** OIDC / Keycloak (confidential client, auth code flow)
- **AI Client:** async-openai (streaming + tool calls)
- **Architecture:** Domain-Driven Design

## Quick Start

### Prerequisites

- Node.js 20+ and pnpm
- Rust (latest stable)
- PostgreSQL 15+
- Qdrant (vector store for embeddings/search)
- Keycloak (or any OIDC-compatible provider)

### Setup

```bash
# Frontend
cd ui
pnpm install

# Backend
cd server
cp .env.example .env
cargo build
```

### Run

```bash
# Frontend dev server (http://localhost:5143)
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
| `CHIROO_DB__ENCRYPTION_KEY` | ✅ | — | 32-byte key for field-level encryption (e.g. `openssl rand 32 \| xxd -p -c 32`) |
| `CHIROO_OPENAI__BASE_URL` | ✅ | — | OpenAI-compatible API endpoint |
| `CHIROO_OPENAI__API_KEY` | ✅ | — | API key |
| `CHIROO_OPENAI__MODEL` | ✅ | — | Model name (e.g. `gpt-4o`) |
| `CHIROO_OPENAI__INCLUDE_USAGE` | | `true` | Request token usage in streaming completion responses |
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

The frontend requires no environment variables. All API communication goes through the Vite proxy to the backend. The `.env.example` file is a legacy artifact and not needed for development.

Product branding (name, label, tagline, colors, logos, fonts) is fetched from the backend (`GET /api/branding`) before the app is mounted; the frontend keeps no branding assets or defaults of its own.

## License

MIT
