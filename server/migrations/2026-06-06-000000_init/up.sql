CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sso_id TEXT NOT NULL UNIQUE,
    sso_provider TEXT NOT NULL,
    name TEXT,
    username TEXT NOT NULL,
    email TEXT,
    avatar_url TEXT,
    admin BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX users_username_key ON users (username);

CREATE TABLE chats (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title VARCHAR NULL,
    owner UUID NOT NULL,
    public BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_sid TEXT NOT NULL UNIQUE,
    access_token BYTEA NOT NULL,
    refresh_token BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE scenarios (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chat_id UUID NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    name VARCHAR NOT NULL DEFAULT '',
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX scenarios_chat_id_idx ON scenarios(chat_id);

CREATE TABLE rounds (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    scenario_id UUID NOT NULL REFERENCES scenarios(id) ON DELETE CASCADE,
    idx INT NOT NULL,
    message JSON NOT NULL,
    sources JSON NOT NULL DEFAULT '[]',
    hidden BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX rounds_scenario_id_idx ON rounds(scenario_id);

CREATE TABLE votes (
    chat_id UUID NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    scenario_id UUID NOT NULL REFERENCES scenarios(id) ON DELETE CASCADE,
    is_upvoted BOOLEAN NOT NULL,
    PRIMARY KEY (chat_id, scenario_id)
);

CREATE TABLE knowledge_bases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    public BOOLEAN NOT NULL DEFAULT FALSE,
    metadata JSON NOT NULL DEFAULT '{}',
    language TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE documents (
    id TEXT PRIMARY KEY,
    kb_id UUID NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    title TEXT NOT NULL DEFAULT '',
    url TEXT NOT NULL DEFAULT '',
    version TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    metadata JSON NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE document_data (
    document_id TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    data BYTEA NOT NULL
);

CREATE INDEX documents_kb_id_idx ON documents(kb_id);
CREATE INDEX documents_kb_id_id_idx ON documents (kb_id, id) INCLUDE (version);
