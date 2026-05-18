CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE chats
(
    id          uuid PRIMARY KEY DEFAULT uuid_generate_v4(),
    title       VARCHAR NULL,
    owner       uuid NOT NULL,
    public      BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
)
