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
        #[max_length = 20]
        role -> Varchar,
        parts -> Json,
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

diesel::joinable!(messages -> chats (chat_id));
diesel::joinable!(votes -> chats (chat_id));
diesel::joinable!(votes -> messages (message_id));

diesel::allow_tables_to_appear_in_same_query!(chats, messages, votes,);
