// @generated automatically by Diesel CLI.

diesel::table! {
    users (id) {
        id -> Int4,
        #[max_length = 128]
        username -> Varchar,
        password -> Varchar,
    }
}
