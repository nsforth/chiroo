CREATE TABLE votes (
    chat_id UUID NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    is_upvoted BOOLEAN NOT NULL,
    PRIMARY KEY (chat_id, message_id)
);
