// @generated automatically by Diesel CLI.

diesel::table! {
    api_tokens (id) {
        id -> Int4,
        user_id -> Int4,
        name -> Varchar,
        #[max_length = 64]
        token_hash -> Varchar,
        create_time -> Timestamptz,
        expires_at -> Nullable<Timestamptz>,
        #[max_length = 45]
        allowed_ip -> Nullable<Varchar>,
    }
}

diesel::table! {
    dns_log (id) {
        id -> Int4,
        #[max_length = 45]
        client_ip -> Varchar,
        client_port -> Int4,
        location -> Varchar,
        query_name -> Varchar,
        #[max_length = 32]
        query_type -> Varchar,
        #[max_length = 32]
        query_class -> Varchar,
        extra_info -> Binary,
        error_log -> Nullable<Text>,
        create_time -> Timestamptz,
    }
}

diesel::table! {
    dns_route (id) {
        id -> Int4,
        pattern_kind -> Int2,
        #[max_length = 1024]
        pattern -> Varchar,
        priority -> Int4,
        timeout -> Int4,
        #[max_length = 1024]
        catalog -> Varchar,
        handler_kind -> Int2,
        handler -> Varchar,
        write_log -> Bool,
        comment -> Text,
        create_time -> Timestamptz,
    }
}

diesel::table! {
    http_log (id) {
        id -> Int4,
        #[max_length = 45]
        client_ip -> Varchar,
        client_port -> Int4,
        location -> Varchar,
        #[max_length = 255]
        method -> Varchar,
        path -> Varchar,
        raw_query -> Varchar,
        parsed_query -> Binary,
        header -> Binary,
        parsed_body_type -> Int2,
        parsed_body -> Binary,
        raw_body -> Binary,
        file -> Binary,
        extra_info -> Binary,
        error_log -> Nullable<Text>,
        create_time -> Timestamptz,
    }
}

diesel::table! {
    http_route (id) {
        id -> Int4,
        pattern_kind -> Int2,
        #[max_length = 1024]
        pattern -> Varchar,
        priority -> Int4,
        timeout -> Int4,
        #[max_length = 1024]
        catalog -> Varchar,
        handler_kind -> Int2,
        handler -> Varchar,
        write_log -> Bool,
        comment -> Text,
        create_time -> Timestamptz,
    }
}

diesel::table! {
    system_log (id) {
        id -> Int4,
        log -> Varchar,
        create_time -> Timestamptz,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        #[max_length = 128]
        username -> Varchar,
        password -> Varchar,
        create_time -> Timestamptz,
    }
}

diesel::joinable!(api_tokens -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    api_tokens, dns_log, dns_route, http_log, http_route, system_log, users,
);
