// @generated automatically by Diesel CLI.

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

// Note: agent_decisions and todos have bidirectional FKs, so we can only define one joinable
diesel::joinable!(todos -> categories (category_id));

diesel::allow_tables_to_appear_in_same_query!(
    agent_decisions,
    calendar_accounts,
    categories,
    email_accounts,
    todos,
);
