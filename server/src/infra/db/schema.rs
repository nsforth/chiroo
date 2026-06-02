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

diesel::table! {
    users (id) {
        id -> Uuid,
        sso_id -> Text,
        sso_provider -> Text,
        name -> Nullable<Text>,
        username -> Nullable<Text>,
        email -> Nullable<Text>,
        avatar_url -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    sessions (id) {
        id -> Uuid,
        user_id -> Uuid,
        provider_sid -> Text,
        access_token -> Text,
        refresh_token -> Text,
        created_at -> Timestamptz,
        expires_at -> Timestamptz,
    }
}

diesel::joinable!(messages -> chats (chat_id));
diesel::joinable!(votes -> chats (chat_id));
diesel::joinable!(votes -> messages (message_id));
diesel::joinable!(sessions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(chats, messages, votes, users, sessions);
