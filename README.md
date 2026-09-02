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
- **Auth:** OIDC / Keycloak (confidential client, auth code flow)
- **AI Client:** async-openai (streaming + tool calls)
- **Architecture:** Domain-Driven Design

## Quick Start

### Prerequisites

- Node.js 20+ and pnpm
- Rust (latest stable)
- PostgreSQL 15+
- Keycloak (or any OIDC-compatible provider)

### Setup

```bash
# Frontend
cd ui
pnpm install

# Backend
cd server
cp .env.example .env   # create your own if missing
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
| `DATABASE_URL` | ✅ | — | PostgreSQL connection string |
| `OPENAI_BASE_URL` | ✅ | — | OpenAI-compatible API endpoint |
| `OPENAI_API_KEY` | ✅ | — | API key |
| `OPENAI_MODEL` | ✅ | — | Model name (e.g. `gpt-4o`) |
| `OIDC_ISSUER_URL` | ✅ | — | OIDC issuer (Keycloak realm URL) |
| `OIDC_CLIENT_ID` | ✅ | — | OIDC client ID |
| `OIDC_CLIENT_SECRET` | ✅ | — | OIDC client secret |
| `STATE_SECRET` | ✅ | — | HMAC key for state/CSRF tokens (min 32 chars) |
| `APP_BASE_URL` | ✅ | — | Public URL of the app (used for OIDC redirect) |
| `HOST` | | `127.0.0.1` | Server bind address |
| `PORT` | | `3000` | Server port |
| `SESSION_CLEANUP_INTERVAL_SECS` | | `600` | Expired session cleanup interval |
| `SESSION_TTL_MULTIPLIER` | | `10` | Session TTL = access_token TTL × multiplier |
| `OIDC_ACCESS_TOKEN_EXPIRES_SECS` | | `300` | Fallback access_token TTL if not in OIDC response |
| `BRAND_NAME` | | `Chiroo` | Product name shown in the UI and browser title |
| `BRAND_TAGLINE` | | — | Optional tagline shown under the product name and used in meta description |
| `BRAND_ASSISTANT_NAME` | | `BRAND_NAME` | Assistant name used in the LLM system prompt |
| `BRAND_PRIMARY_COLOR` | | — | Light-theme accent hex (`#RRGGBB`); must be paired with `BRAND_NEUTRAL_COLOR` |
| `BRAND_NEUTRAL_COLOR` | | — | Light-theme neutral hex — the whole light UI chroma is built from it |
| `BRAND_PRIMARY_DARK_COLOR` | | `BRAND_PRIMARY_COLOR` | Dark-theme accent hex |
| `BRAND_NEUTRAL_DARK_COLOR` | | `BRAND_NEUTRAL_COLOR` | Dark-theme neutral hex (backgrounds/surfaces). If no color is configured, the default Nuxt UI `blue`/`zinc` theme is used |
| `BRAND_LOGO_PATH` | | `assets/branding/logo.png` | Filesystem path to the product logo served by the backend |
| `BRAND_FAVICON_PATH` | | `assets/branding/favicon.png` | Filesystem path to the favicon served by the backend |
| `BRAND_ASSISTANT_ICON_PATH` | | `assets/branding/assistant.png` | Filesystem path to the assistant/mascot icon shown next to the greeting |
| `BRAND_SCENARIO_ICONS_DIR` | | — | Directory with per-scenario icon files (opt-in; combined with `icon_file` on each scenario) |
| `BRAND_SANS_FONT_FAMILY` | | — | CSS family name of the brand sans typeface (pair with `BRAND_SANS_FONT_PATH`) |
| `BRAND_SANS_FONT_PATH` | | — | Filesystem path to a variable font file (`.ttf`/`.otf`/`.woff2`) served for the sans family |
| `BRAND_MONO_FONT_FAMILY` | | — | CSS family name of the brand monospace typeface (pair with `BRAND_MONO_FONT_PATH`) |
| `BRAND_MONO_FONT_PATH` | | — | Filesystem path to a variable font file (`.ttf`/`.otf`/`.woff2`) served for the monospace family |

### Frontend (`ui/`)

The frontend requires no environment variables. All API communication goes through the Vite proxy to the backend. The `.env.example` file is a legacy artifact and not needed for development.

Product branding (name, tagline, logo, favicon, colors) is fetched from the backend (`GET /api/branding`) before the app is mounted; the frontend keeps no branding assets or defaults of its own.

## License

MIT
