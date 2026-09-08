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
cd server && cargo run
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

| Variable | Required | Default | Description |
|---|---|---|---|
| `HOST` | | `127.0.0.1` | Server bind address |
| `PORT` | | `3000` | Server port |
| `INTERNAL_API_HOST` | | `127.0.0.1` | Internal API bind address |
| `INTERNAL_API_PORT` | | `3001` | Internal API port |
| `DATABASE_URL` | ✅ | — | PostgreSQL connection string |
| `DATABASE_ENCRYPTION_KEY` | ✅ | — | 32-byte key for field-level encryption (e.g. `openssl rand 32 \| xxd -p -c 32`) |
| `OPENAI_BASE_URL` | ✅ | — | OpenAI-compatible API endpoint |
| `OPENAI_API_KEY` | ✅ | — | API key |
| `OPENAI_MODEL` | ✅ | — | Model name (e.g. `gpt-4o`) |
| `EMBEDDING_BASE_URL` | | `OPENAI_BASE_URL` | Embeddings API endpoint |
| `EMBEDDING_API_KEY` | | `OPENAI_API_KEY` | Embeddings API key |
| `EMBEDDING_MODEL` | ✅ | — | Embedding model name |
| `EMBEDDING_DIMENSIONS` | ✅ | — | Embedding vector dimensions |
| `RERANK_BASE_URL` | | `OPENAI_BASE_URL` | Reranking API endpoint |
| `RERANK_API_KEY` | | `OPENAI_API_KEY` | Reranking API key |
| `RERANK_MODEL` | ✅ | — | Reranking model name |
| `QDRANT_URL` | | `http://localhost:6334` | Qdrant server URL |
| `QDRANT_API_KEY` | | — | Qdrant API key (if auth is enabled) |
| `QDRANT_COLLECTION_PREFIX` | | `chiroo` | Prefix for Qdrant collection names |
| `QDRANT_TIMEOUT_SECS` | | `30` | Qdrant request timeout |
| `CHUNK_SIZE` | | `512` | Max document chunk size |
| `SEARCH_LIMIT` | | `10` | Max search results returned |
| `SEARCH_SCORE_THRESHOLD` | | `0.5` | Minimum relevance score |
| `SEARCH_CANDIDATE_LIMIT` | | `30` | Max candidates before reranking |
| `APP_BASE_URL` | ✅ | — | Public URL of the app (used for OIDC redirect) |
| `OIDC_ISSUER_URL` | ✅ | — | OIDC issuer (Keycloak realm URL) |
| `OIDC_CLIENT_ID` | ✅ | — | OIDC client ID |
| `OIDC_CLIENT_SECRET` | ✅ | — | OIDC client secret |
| `STATE_SECRET` | ✅ | — | HMAC key for state/CSRF tokens (min 32 chars) |
| `SESSION_CLEANUP_INTERVAL_SECS` | | `600` | Expired session cleanup interval |
| `SESSION_TTL_MULTIPLIER` | | `10` | Session TTL = access_token TTL × multiplier |
| `OIDC_ACCESS_TOKEN_EXPIRES_SECS` | | `300` | Fallback access_token TTL if not in OIDC response |

Product branding is configured through `BRAND_*` variables — see [BRANDING.md](BRANDING.md).

### Frontend (`ui/`)

The frontend requires no environment variables. All API communication goes through the Vite proxy to the backend. The `.env.example` file is a legacy artifact and not needed for development.

Product branding (name, label, tagline, colors, logos, fonts) is fetched from the backend (`GET /api/branding`) before the app is mounted; the frontend keeps no branding assets or defaults of its own.

## License

MIT
