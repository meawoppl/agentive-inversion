// @generated automatically by Diesel CLI.

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
    }
}

diesel::joinable!(todos -> categories (category_id));

diesel::allow_tables_to_appear_in_same_query!(calendar_accounts, categories, email_accounts, todos,);
