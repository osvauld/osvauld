// @generated automatically by Diesel CLI.

diesel::table! {
    credentials (id) {
        id -> Text,
        credential_type -> Text,
        data -> Text,
        folder_id -> Text,
        signature -> Text,
        encrypted_key -> Text,
        favorite -> Bool,
        last_accessed -> BigInt,
        deleted -> Bool,
        deleted_at -> Nullable<BigInt>,
        updated_at -> BigInt,
        created_at -> BigInt,
    }
}

diesel::table! {
    devices (id) {
        id -> Text,
        device_key -> Text,
        updated_at -> BigInt,
        created_at -> BigInt,
        last_synced_at -> Nullable<BigInt>,
    }
}

diesel::table! {
    folders (id) {
        id -> Text,
        name -> Text,
        description -> Nullable<Text>,
        deleted -> Bool,
        deleted_at -> Nullable<BigInt>,
        updated_at -> BigInt,
        created_at -> BigInt,
    }
}

diesel::table! {
    sync_records (id) {
        id -> Text,
        resource_id -> Text,
        resource_type -> Text,
        operation_type -> Text,
        folder_id -> Nullable<Text>,
        credential_id -> Nullable<Text>,
        source_device_id -> Text,
        target_device_id -> Text,
        status -> Text,
        synced_from -> Nullable<Text>,
        updated_at -> BigInt,
        created_at -> BigInt,
    }
}

diesel::table! {
    users (id) {
        id -> Text,
        username -> Text,
        public_key -> Text,
        updated_at -> BigInt,
        created_at -> BigInt,
    }
}

diesel::joinable!(credentials -> folders (folder_id));

diesel::allow_tables_to_appear_in_same_query!(
    credentials,
    devices,
    folders,
    sync_records,
    users,
);
