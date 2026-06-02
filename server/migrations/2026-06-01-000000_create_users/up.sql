CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sso_id TEXT NOT NULL UNIQUE,
    sso_provider TEXT NOT NULL,
    name TEXT,
    username TEXT,
    email TEXT,
    avatar_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
