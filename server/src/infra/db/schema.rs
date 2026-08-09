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
        access_token -> Binary,
        refresh_token -> Binary,
        created_at -> Timestamptz,
        expires_at -> Timestamptz,
    }
}

diesel::table! {
    knowledge_bases (id) {
        id -> Uuid,
        name -> Text,
        description -> Text,
        public -> Bool,
        metadata -> Json,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    documents (id) {
        id -> Text,
        kb_id -> Uuid,
        title -> Text,
        url -> Text,
        version -> Text,
        mime_type -> Text,
        metadata -> Json,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    document_data (document_id) {
        document_id -> Text,
        data -> Binary,
    }
}

diesel::joinable!(messages -> chats (chat_id));
diesel::joinable!(votes -> chats (chat_id));
diesel::joinable!(votes -> messages (message_id));
diesel::joinable!(sessions -> users (user_id));
diesel::joinable!(documents -> knowledge_bases (kb_id));
diesel::joinable!(document_data -> documents (document_id));

diesel::allow_tables_to_appear_in_same_query!(
    chats,
    messages,
    votes,
    users,
    sessions,
    knowledge_bases,
    documents,
    document_data,
);
