// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "chat_state_enum"))]
    pub struct ChatStateEnum;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "status_enum"))]
    pub struct StatusEnum;

    #[derive(diesel::query_builder::QueryId, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "user_type_enum"))]
    pub struct UserTypeEnum;
}

diesel::table! {
    invites (id) {
        id -> Uuid,
        created_at -> Timestamp,
        token -> Text,
        is_used -> Bool,
        owner_id -> Nullable<Uuid>,
        user_id -> Nullable<Uuid>,
    }
}

diesel::table! {
    ntfy_users (id) {
        id -> Uuid,
        enabled -> Bool,
        topic -> Text,
        topic_permission -> Text,
        username -> Text,
        password -> Text,
        tier -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ChatStateEnum;

    tg_users (id) {
        id -> Uuid,
        enabled -> Bool,
        user_id -> Int8,
        chat_id -> Nullable<Int8>,
        chat_state -> ChatStateEnum,
        language_code -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::StatusEnum;

    uptime_states (id) {
        id -> Uuid,
        created_at -> Timestamp,
        touched_at -> Timestamp,
        status -> StatusEnum,
        user_id -> Nullable<Uuid>,
        state_changed_at -> Timestamp,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::UserTypeEnum;

    users (id) {
        id -> Uuid,
        created_at -> Timestamp,
        user_type -> UserTypeEnum,
        invites_limit -> Int8,
        invites_used -> Int8,
        access_token -> Text,
        up_delay -> Int2,
        down_delay -> Int2,
        ntfy_id -> Uuid,
        tg_id -> Uuid,
    }
}

diesel::joinable!(uptime_states -> users (user_id));
diesel::joinable!(users -> ntfy_users (ntfy_id));
diesel::joinable!(users -> tg_users (tg_id));

diesel::allow_tables_to_appear_in_same_query!(
    invites,
    ntfy_users,
    tg_users,
    uptime_states,
    users,
);
