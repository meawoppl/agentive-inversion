// @generated automatically by Diesel CLI.

diesel::table! {
    sources (id) {
        id -> Uuid,
        user_id -> Uuid,
        #[max_length = 200]
        name -> Varchar,
        #[max_length = 255]
        email -> Nullable<Varchar>,
        #[max_length = 255]
        calendar_id -> Nullable<Varchar>,
        credentials_encrypted -> Bytea,
        polling_interval_seconds -> Int4,
        last_polled_at -> Nullable<Timestamp>,
        enabled -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    sync_logs (id) {
        id -> Uuid,
        source_id -> Uuid,
        started_at -> Timestamp,
        completed_at -> Nullable<Timestamp>,
        #[max_length = 50]
        status -> Varchar,
        items_processed -> Nullable<Int4>,
        items_created -> Nullable<Int4>,
        items_updated -> Nullable<Int4>,
        error_message -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    todos (id) {
        id -> Uuid,
        user_id -> Uuid,
        #[max_length = 500]
        title -> Varchar,
        description -> Nullable<Text>,
        source_id -> Nullable<Uuid>,
        source_url -> Nullable<Text>,
        #[max_length = 255]
        external_id -> Nullable<Varchar>,
        due_date -> Nullable<Timestamp>,
        completed -> Bool,
        completed_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        #[max_length = 255]
        email -> Varchar,
        #[max_length = 255]
        name -> Nullable<Varchar>,
        #[max_length = 255]
        google_id -> Nullable<Varchar>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::joinable!(sources -> users (user_id));
diesel::joinable!(sync_logs -> sources (source_id));
diesel::joinable!(todos -> sources (source_id));
diesel::joinable!(todos -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    sources,
    sync_logs,
    todos,
    users,
);
