// @generated automatically by Diesel CLI.

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
        default_folder -> Bool,
        deleted -> Bool,
        deleted_at -> Nullable<BigInt>,
        updated_at -> BigInt,
        created_at -> BigInt,
    }
}

diesel::table! {
    resource_keys (id) {
        id -> Text,
        resource_id -> Text,
        user_id -> Text,
        encrypted_key -> Text,
        is_owner -> Bool,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    resources (id) {
        id -> Text,
        resource_type -> Text,
        data -> Text,
        folder_id -> Text,
        signature -> Text,
        favourite -> Bool,
        last_accessed -> BigInt,
        deleted -> Bool,
        deleted_at -> Nullable<BigInt>,
        updated_at -> BigInt,
        created_at -> BigInt,
        vector_clock -> Text,
    }
}

diesel::table! {
    share_records (id) {
        id -> Text,
        resource_id -> Text,
        shared_by_user_id -> Text,
        operation_type -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    store_items (key) {
        key -> Text,
        value -> Text,
        updated_at -> BigInt,
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
    user_record_status (id) {
        id -> Text,
        user_record_id -> Text,
        aware_user_id -> Text,
        synced -> Bool,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    user_records (id) {
        id -> Text,
        share_record_id -> Text,
        user_id -> Text,
        status -> Text,
        synced -> Bool,
        created_at -> BigInt,
        updated_at -> BigInt,
    }
}

diesel::table! {
    users (id) {
        id -> Text,
        username -> Text,
        public_key -> Text,
        updated_at -> BigInt,
        created_at -> BigInt,
        signature -> Text,
        owner -> Bool,
        deleted -> Bool,
        deleted_at -> Nullable<BigInt>,
    }
}

diesel::joinable!(device_record_status -> device_records (device_record_id));
diesel::joinable!(device_record_status -> devices (aware_device_id));
diesel::joinable!(device_records -> devices (device_id));
diesel::joinable!(device_records -> sync_records (sync_record_id));
diesel::joinable!(resource_keys -> resources (resource_id));
diesel::joinable!(resource_keys -> users (user_id));
diesel::joinable!(resources -> folders (folder_id));
diesel::joinable!(share_records -> resources (resource_id));
diesel::joinable!(share_records -> users (shared_by_user_id));
diesel::joinable!(sync_records -> devices (source_device_id));
diesel::joinable!(user_record_status -> user_records (user_record_id));
diesel::joinable!(user_record_status -> users (aware_user_id));
diesel::joinable!(user_records -> share_records (share_record_id));
diesel::joinable!(user_records -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    device_record_status,
    device_records,
    devices,
    folders,
    resource_keys,
    resources,
    share_records,
    store_items,
    sync_records,
    user_record_status,
    user_records,
    users,
);
