// @generated automatically by Diesel CLI.

diesel::table! {
    agent_rules (id) {
        id -> Uuid,
        name -> Varchar,
        description -> Nullable<Text>,
        source_type -> Varchar,
        rule_type -> Varchar,
        conditions -> Text,
        action -> Varchar,
        action_params -> Nullable<Text>,
        priority -> Int4,
        is_active -> Bool,
        created_from_decision_id -> Nullable<Uuid>,
        match_count -> Int4,
        last_matched_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    agent_decisions (id) {
        id -> Uuid,
        source_type -> Varchar,
        source_id -> Nullable<Uuid>,
        source_external_id -> Nullable<Varchar>,
        decision_type -> Varchar,
        proposed_action -> Text,
        reasoning -> Text,
        reasoning_details -> Nullable<Text>,
        confidence -> Float4,
        status -> Varchar,
        applied_rule_id -> Nullable<Uuid>,
        result_todo_id -> Nullable<Uuid>,
        user_feedback -> Nullable<Text>,
        created_at -> Timestamptz,
        reviewed_at -> Nullable<Timestamptz>,
        executed_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    calendar_accounts (id) {
        id -> Uuid,
        account_name -> Varchar,
        calendar_id -> Varchar,
        last_synced -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    categories (id) {
        id -> Uuid,
        name -> Varchar,
        color -> Nullable<Varchar>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    email_accounts (id) {
        id -> Uuid,
        account_name -> Varchar,
        email_address -> Varchar,
        provider -> Varchar,
        last_synced -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        oauth_refresh_token -> Nullable<Text>,
        oauth_access_token -> Nullable<Text>,
        oauth_token_expires_at -> Nullable<Timestamptz>,
        last_message_id -> Nullable<Varchar>,
        sync_status -> Varchar,
        last_sync_error -> Nullable<Text>,
        is_active -> Bool,
    }
}

diesel::table! {
    emails (id) {
        id -> Uuid,
        account_id -> Uuid,
        #[max_length = 255]
        gmail_id -> Varchar,
        #[max_length = 255]
        thread_id -> Varchar,
        history_id -> Nullable<Int8>,
        subject -> Text,
        #[max_length = 255]
        from_address -> Varchar,
        #[max_length = 255]
        from_name -> Nullable<Varchar>,
        to_addresses -> Array<Nullable<Text>>,
        cc_addresses -> Nullable<Array<Nullable<Text>>>,
        snippet -> Nullable<Text>,
        body_text -> Nullable<Text>,
        body_html -> Nullable<Text>,
        labels -> Nullable<Array<Nullable<Text>>>,
        has_attachments -> Bool,
        received_at -> Timestamptz,
        fetched_at -> Timestamptz,
        processed -> Bool,
        processed_at -> Nullable<Timestamptz>,
        archived_in_gmail -> Bool,
    }
}

diesel::table! {
    todos (id) {
        id -> Uuid,
        title -> Varchar,
        description -> Nullable<Text>,
        completed -> Bool,
        source -> Varchar,
        source_id -> Nullable<Varchar>,
        due_date -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        link -> Nullable<Varchar>,
        category_id -> Nullable<Uuid>,
        decision_id -> Nullable<Uuid>,
    }
}

diesel::joinable!(emails -> email_accounts (account_id));
diesel::joinable!(todos -> categories (category_id));

diesel::allow_tables_to_appear_in_same_query!(
    agent_rules,
    agent_decisions,
    calendar_accounts,
    categories,
    email_accounts,
    emails,
    todos,
);
