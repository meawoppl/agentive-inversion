// @generated automatically by Diesel CLI.

diesel::table! {
    agent_decisions (id) {
        id -> Uuid,
        #[max_length = 50]
        source_type -> Varchar,
        source_id -> Nullable<Uuid>,
        #[max_length = 255]
        source_external_id -> Nullable<Varchar>,
        #[max_length = 50]
        decision_type -> Varchar,
        proposed_action -> Text,
        reasoning -> Text,
        reasoning_details -> Nullable<Text>,
        confidence -> Float4,
        #[max_length = 50]
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
    agent_rules (id) {
        id -> Uuid,
        #[max_length = 255]
        name -> Varchar,
        description -> Nullable<Text>,
        #[max_length = 50]
        source_type -> Varchar,
        #[max_length = 50]
        rule_type -> Varchar,
        conditions -> Text,
        #[max_length = 50]
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
    calendar_events (id) {
        id -> Uuid,
        account_id -> Uuid,
        #[max_length = 255]
        google_event_id -> Varchar,
        #[max_length = 255]
        ical_uid -> Nullable<Varchar>,
        summary -> Nullable<Text>,
        description -> Nullable<Text>,
        location -> Nullable<Text>,
        start_time -> Timestamptz,
        end_time -> Timestamptz,
        all_day -> Bool,
        recurring -> Bool,
        recurrence_rule -> Nullable<Text>,
        #[max_length = 50]
        status -> Varchar,
        #[max_length = 255]
        organizer_email -> Nullable<Varchar>,
        attendees -> Nullable<Text>,
        conference_link -> Nullable<Text>,
        fetched_at -> Timestamptz,
        processed -> Bool,
        processed_at -> Nullable<Timestamptz>,
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
    chat_messages (id) {
        id -> Uuid,
        #[max_length = 20]
        role -> Varchar,
        content -> Text,
        #[max_length = 50]
        intent -> Nullable<Varchar>,
        created_at -> Timestamptz,
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
    google_accounts (id) {
        id -> Uuid,
        email -> Varchar,
        name -> Nullable<Varchar>,
        refresh_token -> Text,
        access_token -> Nullable<Text>,
        token_expires_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
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

diesel::joinable!(calendar_events -> google_accounts (account_id));
diesel::joinable!(emails -> google_accounts (account_id));
diesel::joinable!(todos -> categories (category_id));

diesel::allow_tables_to_appear_in_same_query!(
    agent_decisions,
    agent_rules,
    calendar_events,
    categories,
    chat_messages,
    emails,
    google_accounts,
    todos,
);
