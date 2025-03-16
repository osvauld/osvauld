CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE,
    public_key TEXT NOT NULL,
    updated_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    signature TEXT NOT NULL,
    owner BOOLEAN NOT NULL DEFAULT FALSE,
    deleted BOOLEAN NOT NULL,
    deleted_at BIGINT
);

CREATE TABLE folders (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    default_folder BOOLEAN NOT NULL DEFAULT FALSE,
    deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at BIGINT,
    updated_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL 
);


CREATE TABLE devices (
    id TEXT PRIMARY KEY NOT NULL,
    device_key TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL,
    updated_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    last_synced_at BIGINT,
    FOREIGN KEY (user_id) REFERENCES users (id)
);

CREATE TABLE sync_records (
    id TEXT PRIMARY KEY NOT NULL,
    resource_id TEXT NOT NULL,           -- ID of the folder/credential being synced
    resource_type TEXT NOT NULL,         -- 'folder', 'credential', etc.
    operation_type TEXT NOT NULL,        -- 'create', 'update', 'delete', etc.
    source_device_id TEXT NOT NULL,      -- Device that initiated the sync
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (source_device_id) REFERENCES devices (id)
);

CREATE TABLE device_records (
    id TEXT PRIMARY KEY NOT NULL,
    sync_record_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    status TEXT NOT NULL,                -- 'pending', 'completed', 'failed'
    synced BOOLEAN NOT NULL DEFAULT FALSE,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (sync_record_id) REFERENCES sync_records (id),
    FOREIGN KEY (device_id) REFERENCES devices (id)
);

CREATE TABLE device_record_status (
    id TEXT PRIMARY KEY NOT NULL,
    device_record_id TEXT NOT NULL,
    aware_device_id TEXT NOT NULL,       -- Device that knows about this sync status
    synced BOOLEAN NOT NULL DEFAULT FALSE,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (device_record_id) REFERENCES device_records (id),
    FOREIGN KEY (aware_device_id) REFERENCES devices (id)
);

CREATE TABLE resources (
    id TEXT PRIMARY KEY NOT NULL,
    resource_type TEXT NOT NULL,
    data TEXT NOT NULL,
    folder_id TEXT NOT NULL,
    signature TEXT NOT NULL,
    favourite BOOLEAN NOT NULL DEFAULT FALSE,
    last_accessed BIGINT NOT NULL,
    deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at BIGINT,
    updated_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
        vector_clock TEXT NOT NULL DEFAULT '{"clock":{}}',
    FOREIGN KEY (folder_id) REFERENCES folders (id)
);

-- Resource keys table for per-user encryption keys
CREATE TABLE resource_keys (
    id TEXT PRIMARY KEY NOT NULL,
    resource_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    encrypted_key TEXT NOT NULL,
    is_owner BOOLEAN NOT NULL DEFAULT FALSE,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (resource_id) REFERENCES resources(id),
    FOREIGN KEY (user_id) REFERENCES users(id),
    UNIQUE(resource_id, user_id)
);

CREATE TABLE share_records (
    id TEXT PRIMARY KEY NOT NULL,
    resource_id TEXT NOT NULL,           -- ID of the resource being shared
    shared_by_user_id TEXT NOT NULL,     -- User who initiated the share
    operation_type TEXT NOT NULL,        -- 'share', 'revoke', 'update_permission'
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (resource_id) REFERENCES resources (id),
    FOREIGN KEY (shared_by_user_id) REFERENCES users (id)
);

CREATE TABLE user_records (
    id TEXT PRIMARY KEY NOT NULL,
    share_record_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    status TEXT NOT NULL,                -- 'pending', 'accepted', 'rejected', 'completed'
    synced BOOLEAN NOT NULL DEFAULT FALSE,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (share_record_id) REFERENCES share_records (id),
    FOREIGN KEY (user_id) REFERENCES users (id)
);

CREATE TABLE user_record_status (
    id TEXT PRIMARY KEY NOT NULL,
    user_record_id TEXT NOT NULL,
    aware_user_id TEXT NOT NULL,         -- User that knows about this share status
    synced BOOLEAN NOT NULL DEFAULT FALSE,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (user_record_id) REFERENCES user_records (id),
    FOREIGN KEY (aware_user_id) REFERENCES users (id)
);
CREATE TABLE store_items (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);
