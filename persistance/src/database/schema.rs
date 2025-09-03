// @generated automatically by Diesel CLI.

diesel::table! {
    devices (id) {
        id -> Text,
        device_key -> Text,
        user_id -> Text,
        updated_at -> BigInt,
        created_at -> BigInt,
        last_synced_at -> Nullable<BigInt>,
    }
}

diesel::table! {
    folder_share_records (id) {
        id -> Text,
        folder_id -> Text,
        shared_by_user_id -> Text,
        recipient_user_id -> Text,
        permission_level -> Text,
        ucan_token -> Text,
        ucan_cid -> Text,
        operation_type -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
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
    resource_vector_clocks (id) {
        id -> Text,
        resource_id -> Text,
        device_id -> Text,
        clock_value -> Integer,
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
        created_by -> Text,
        last_accessed -> BigInt,
        deleted -> Bool,
        deleted_at -> Nullable<BigInt>,
        updated_at -> BigInt,
        created_at -> BigInt,
    }
}

diesel::table! {
    share_records (id) {
        id -> Text,
        resource_id -> Text,
        shared_by_user_id -> Text,
        recipient_user_id -> Text,
        permission_level -> Text,
        ucan_token -> Text,
        ucan_cid -> Text,
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
    users (id) {
        id -> Text,
        username -> Text,
        public_key -> Text,
        updated_at -> BigInt,
        created_at -> BigInt,
        signature -> Text,
        ucan_token -> Text,
        ucan_pub_key -> Text,
        ucan_cid -> Text,
        owner -> Bool,
        first_sync -> Bool,
        deleted -> Bool,
        deleted_at -> Nullable<BigInt>,
    }
}

diesel::joinable!(devices -> users (user_id));
diesel::joinable!(folder_share_records -> folders (folder_id));
diesel::joinable!(resource_keys -> resources (resource_id));
diesel::joinable!(resource_keys -> users (user_id));
diesel::joinable!(resource_vector_clocks -> devices (device_id));
diesel::joinable!(resource_vector_clocks -> resources (resource_id));
diesel::joinable!(resources -> folders (folder_id));
diesel::joinable!(resources -> users (created_by));
diesel::joinable!(share_records -> resources (resource_id));

diesel::allow_tables_to_appear_in_same_query!(
    devices,
    folder_share_records,
    folders,
    resource_keys,
    resource_vector_clocks,
    resources,
    share_records,
    store_items,
    users,
);
