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
    scenarios (id) {
        id -> Uuid,
        chat_id -> Uuid,
        #[max_length = 255]
        name -> Varchar,
        #[max_length = 20]
        status -> Varchar,
        token_usage -> Nullable<Json>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    rounds (id) {
        id -> Uuid,
        scenario_id -> Uuid,
        idx -> Int4,
        message -> Json,
        sources -> Json,
        hidden -> Bool,
        token_usage -> Nullable<Json>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    votes (chat_id, scenario_id) {
        chat_id -> Uuid,
        scenario_id -> Uuid,
        is_upvoted -> Bool,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        sso_id -> Text,
        sso_provider -> Text,
        name -> Nullable<Text>,
        username -> Text,
        email -> Nullable<Text>,
        avatar_url -> Nullable<Text>,
        admin -> Bool,
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
        language -> Nullable<Text>,
        indexes -> Nullable<Json>,
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

diesel::joinable!(scenarios -> chats (chat_id));
diesel::joinable!(rounds -> scenarios (scenario_id));
diesel::joinable!(votes -> chats (chat_id));
diesel::joinable!(votes -> scenarios (scenario_id));
diesel::joinable!(sessions -> users (user_id));
diesel::joinable!(documents -> knowledge_bases (kb_id));
diesel::joinable!(document_data -> documents (document_id));

diesel::allow_tables_to_appear_in_same_query!(
    chats,
    scenarios,
    rounds,
    votes,
    users,
    sessions,
    knowledge_bases,
    documents,
    document_data,
);
