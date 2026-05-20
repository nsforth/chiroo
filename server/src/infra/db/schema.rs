// @generated manually - run `diesel setup && diesel migration run` to auto-generate

diesel::table! {
    chats (id) {
        id -> Uuid,
        title -> Nullable<Varchar>,
        owner -> Uuid,
        public -> Bool,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    messages (id) {
        id -> Uuid,
        chat_id -> Uuid,
        role -> Varchar,
        parts -> Jsonb,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    votes (chat_id, message_id) {
        chat_id -> Uuid,
        message_id -> Uuid,
        is_upvoted -> Bool,
    }
}

diesel::allow_tables_to_appear_in_same_query!(chats, messages, votes);
