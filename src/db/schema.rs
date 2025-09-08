// @generated automatically by Diesel CLI.

diesel::table! {
    http_log (id) {
        id -> Int4,
        #[max_length = 45]
        client_ip -> Varchar,
        client_port -> Int4,
        #[max_length = 255]
        method -> Varchar,
        path -> Varchar,
        arg -> Jsonb,
        header -> Jsonb,
        body_type -> Int2,
        body -> Text,
        file -> Jsonb,
        create_time -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        #[max_length = 128]
        username -> Varchar,
        password -> Varchar,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    http_log,
    users,
);
