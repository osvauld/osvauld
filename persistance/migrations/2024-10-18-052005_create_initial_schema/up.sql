CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE,
    public_key TEXT NOT NULL,
    updated_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    signature TEXT NOT NULL,
    ucan_token TEXT NOT NULL,
    ucan_pub_key TEXT NOT NULL,
    ucan_cid TEXT NOT NULL,
    owner BOOLEAN NOT NULL DEFAULT FALSE,
    first_sync BOOLEAN NOT NULL DEFAULT FALSE,
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

CREATE TABLE resources (
    id TEXT PRIMARY KEY NOT NULL,
    resource_type TEXT NOT NULL,
    data TEXT NOT NULL,
    folder_id TEXT NOT NULL,
    signature TEXT NOT NULL,
    favourite BOOLEAN NOT NULL DEFAULT FALSE,
    created_by TEXT NOT NULL,
    last_accessed BIGINT NOT NULL,
    deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_at BIGINT,
    updated_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    FOREIGN KEY (folder_id) REFERENCES folders (id)
    FOREIGN KEY (created_by) REFERENCES users (id)
);

CREATE TABLE resource_vector_clocks (
    id TEXT PRIMARY KEY NOT NULL,
    resource_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    clock_value INTEGER NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (resource_id) REFERENCES resources (id),
    FOREIGN KEY (device_id) REFERENCES devices (id),
    UNIQUE (resource_id, device_id)
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
    resource_id TEXT NOT NULL,          
    shared_by_user_id TEXT NOT NULL,    
    recipient_user_id TEXT NOT NULL,    
    permission_level TEXT NOT NULL,     
    ucan_token TEXT NOT NULL,            
    ucan_cid TEXT NOT NULL,
    operation_type TEXT NOT NULL,       
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (resource_id) REFERENCES resources (id),
    FOREIGN KEY (shared_by_user_id) REFERENCES users (id),
    FOREIGN KEY (recipient_user_id) REFERENCES users (id)
);


CREATE TABLE store_items (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE TABLE folder_share_records (
    id TEXT PRIMARY KEY NOT NULL,
    folder_id TEXT NOT NULL,
    shared_by_user_id TEXT NOT NULL,
    recipient_user_id TEXT NOT NULL,
    permission_level TEXT NOT NULL,
    ucan_token TEXT NOT NULL,
    ucan_cid TEXT NOT NULL,
    operation_type TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (folder_id) REFERENCES folders (id),
    FOREIGN KEY (shared_by_user_id) REFERENCES users (id),
    FOREIGN KEY (recipient_user_id) REFERENCES users (id)
);
