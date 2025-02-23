// @generated automatically by Diesel CLI.

diesel::table! {
    credentials (id) {
        id -> Text,
        credential_type -> Text,
        data -> Text,
        folder_id -> Text,
        signature -> Text,
        encrypted_key -> Text,
        favourite -> Bool,
        last_accessed -> BigInt,
        deleted -> Bool,
        deleted_at -> Nullable<BigInt>,
        updated_at -> BigInt,
        created_at -> BigInt,
    }
}

diesel::table! {
    device_record_status (id) {
        id -> Text,
        device_record_id -> Text,
        aware_device_id -> Text,
        synced -> Bool,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    device_records (id) {
        id -> Text,
        sync_record_id -> Text,
        device_id -> Text,
        status -> Text,
        synced -> Bool,
        created_at -> BigInt,
        updated_at -> BigInt,
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
        source_device_id -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    users (id) {
        id -> Text,
        username -> Text,
        public_key -> Text,
        owner -> Bool,
        updated_at -> BigInt,
        created_at -> BigInt,
    }
}

diesel::joinable!(credentials -> folders (folder_id));
diesel::joinable!(device_record_status -> device_records (device_record_id));
diesel::joinable!(device_record_status -> devices (aware_device_id));
diesel::joinable!(device_records -> devices (device_id));
diesel::joinable!(device_records -> sync_records (sync_record_id));
diesel::joinable!(sync_records -> devices (source_device_id));

diesel::allow_tables_to_appear_in_same_query!(
    credentials,
    device_record_status,
    device_records,
    devices,
    folders,
    sync_records,
    users,
);
