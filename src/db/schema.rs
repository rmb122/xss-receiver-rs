// @generated automatically by Diesel CLI.

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
        arg -> Jsonb,
        header -> Jsonb,
        body_type -> Int2,
        body -> Text,
        file -> Jsonb,
        extra_info -> Jsonb,
        error_log -> Nullable<Text>,
        create_time -> Timestamptz,
    }
}

diesel::table! {
    route (id) {
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

diesel::allow_tables_to_appear_in_same_query!(
    http_log,
    route,
    system_log,
    users,
);
