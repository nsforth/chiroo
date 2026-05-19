# Chiroo

AI-чатбот с полным стеком: Vue 3 фронтенд и Rust бэкенд.

## Архитектура

```
chiroo/
├── ui/          # Frontend (Vue 3 + Vite)
│   └── src/     # Исходный код фронтенда
│       ├── composables/  # Composables (состояние, действия)
│       ├── components/   # Vue-компоненты
│       ├── pages/        # Маршруты Vue Router
│       ├── tools/        # Инструменты AI SDK (weather, chart)
│       └── types/        # Типы данных (session, drizzle)
└── server/      # Backend (Rust + Axum)
    ├── src/     # Исходный код бэкенда
    └── migrations/ # Миграции Diesel ORM
```

## Стек технологий

### Frontend (`ui/`)

- **Фреймворк:** Vue 3, Vite
- **UI:** Nuxt UI, Tailwind CSS
- **AI SDK:** Vercel AI SDK — стриминг сообщений от Claude, Gemini, GPT
- **Роутинг:** Vue Router
- **Локализация:** vue-i18n (EN, RU)

### Backend (`server/`)

- **Фреймворк:** Axum (Tokio)
- **ORM:** Diesel
- **База данных:** PostgreSQL
- **Архитектура:** Domain-Driven Design (DDD)

## Быстрый старт

### Предварительные требования

- Node.js 20+ и pnpm
- Rust (последняя стабильная версия)
- PostgreSQL

### Установка

```bash
# Frontend
cd ui
pnpm install
pnpm db:migrate

# Backend
cd server
cp .env.example .env
cargo build
```

### Запуск

```bash
# Frontend (http://localhost:5143)
cd ui && pnpm dev

# Backend (http://127.0.0.1:3000)
cd server && cargo run
```

## Команды

| Команда | Описание |
|---|---|
| `pnpm dev` (ui) | Запуск dev-сервера фронтенда |
| `pnpm build` (ui) | Сборка для продакшена |
| `pnpm typecheck` (ui) | Проверка типов TypeScript |
| `pnpm lint` (ui) | ESLint проверка кода |
| `cargo run` (server) | Запуск бэкенда в режиме разработки |
| `cargo build --release` (server) | Сборка бэкенда для продакшена |

## Конфигурация

### Frontend

Скопируйте `.env.example` → `.env` и настройте:

```env
SESSION_SECRET=<секрет, минимум 32 символа>
```

### Backend

В файле `.env` настройте:

```env
DATABASE_URL=postgres://user:password@localhost/your_db_name
SERVER_HOST=127.0.0.1
SERVER_PORT=3000
```

## Лицензия

MIT
