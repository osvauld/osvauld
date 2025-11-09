# Function Usage Tracker - Sthalam

**Purpose**: Track which backend functions are actually being used in the frontend to facilitate cleanup at the end of the Loro migration.

**Last Updated**: 2025-11-07

---

## Exclusions

❌ **Auth functions are excluded** from tracking (they're core functionality):
- `isSignedUp` → `check_signup_status`
- `savePassphrase` → `handle_sign_up`
- `checkPvtLoaded` → `check_private_key_loaded`
- `login` → `login`
- `addDevice` → `handle_add_device`
- `exportCertificate` → `handle_export_certificate`
- `changePassphrase` → `handle_change_passphrase`
- `logout` → `handle_logout`
- `getUserDetails` → `get_user_details`
- `getOneTimeUcanToken` → `get_one_time_ucan_token`

---

## Functions to Track

### 📁 Folder Functions

| Frontend Action | Backend Handler | Status | Used In | Notes |
|----------------|-----------------|--------|---------|-------|
| `addFolder` | `handle_add_folder` | ✅ ACTIVE | FolderManager.svelte:26, data.svelte.ts:164 | Create new folder - [See flow](#addFolder-flow) |
| `getFolder` | `handle_get_folders` | ✅ ACTIVE | data.svelte.ts:119 | Get all folders - [See flow](#getFolder-flow) |
| `deleteFolder` | `handle_soft_delete_folder` | ⏳ TODO | | Soft delete folder |
| `shareFolder` | `handle_share_folder` | ✅ ACTIVE | PublishWebsiteModal.svelte:132 | Share folder with node - [See flow](#shareFolder-flow) |
| `getSharedFolderUsers` | `handle_get_shared_folder_users` | ⏳ TODO | | Get users with folder access |

### 📄 Resource Functions

| Frontend Action | Backend Handler | Status | Used In | Notes |
|----------------|-----------------|--------|---------|-------|
| `addCredential` | `handle_add_resource` | ✅ ACTIVE | data.svelte.ts:252 | Create new resource - [See flow](#addresource-flow) |
| `getCredential` | `handle_get_resource` | ✅ ACTIVE | data.svelte.ts:326 | Get single resource - [See flow](#getCredential-flow) |
| `getAllResourcesMetadata` | `handle_get_all_resources_metadata` | ✅ ACTIVE | data.svelte.ts:133 | Get all resources metadata - [See flow](#getAllResourcesMetadata-flow) |
| `updateCredential` | `handle_update_resource` | ✅ ACTIVE | data.svelte.ts:327, 441 | Update resource with AES key rotation - [See flow](#updateCredential-flow) |
| `deleteResource` | `soft_delete_resource` | ⏳ TODO | | Soft delete resource |
| `getCredentialsForFolder` | `handle_get_resources_for_folder` | ⏳ TODO | | Get resources in folder |
| `getAllCredentials` | `handle_get_all_resources` | ⏳ TODO | | Get all resources |
| `toggleFav` | `handle_toggle_fav` | ⏳ TODO | | Toggle favorite status |
| `updateLastAccessed` | `handle_update_last_accessed` | ⏳ TODO | | Update last accessed time |
| `searchResource` | `handle_search_resources` | ⏳ TODO | | Search resources |
| `shareResource` | `handle_share_resource` | ⏳ TODO | | Share resource with user |
| `publishResource` | `handle_publish_resource` | ⏳ TODO | | Publish resource |

### 👥 User Functions

| Frontend Action | Backend Handler | Status | Used In | Notes |
|----------------|-----------------|--------|---------|-------|
| `addKnownUser` | `handle_add_user` | ⏳ TODO | | Add known user |
| `getKnownUsers` | `handle_get_known_users` | ⏳ TODO | | Get all known users |

### 🌐 Website Functions (May be removed)

| Frontend Action | Backend Handler | Status | Used In | Notes |
|----------------|-----------------|--------|---------|-------|
| `connectToWebsite` | `handle_connect_to_website` | ❌ REMOVED | | Website feature |
| `syncResource` | `handle_sync_resource` | ❌ REMOVED | | Website feature |
| `folderSyncViewer` | `handle_folder_sync_viewer` | ❌ REMOVED | | Website feature |

### 🌐 P2P Functions

| Frontend Action | Backend Handler | Status | Used In | Notes |
|----------------|-----------------|--------|---------|-------|
| `startP2PListener` | `start_p2p_listener` | ✅ ACTIVE | (Called after login) | Initialize P2P network - [See flow](#startP2PListener-flow) |
| `addSovereignNode` | `handle_add_sovereign_node` | ✅ ACTIVE | (Manual action) | Add sovereign node connection - [See flow](#addSovereignNode-flow) |

### 🔄 Live Edit Functions (Removed - Loro migration)

| Frontend Action | Backend Handler | Status | Used In | Notes |
|----------------|-----------------|--------|---------|-------|
| `updateCurrentNote` | `update_current_note` | ❌ REMOVED | | Old live edit |
| `emitAllResources` | `emit_all_resources` | ❌ REMOVED | | Old live edit |

---

## Status Legend

- ⏳ **TODO**: Not yet implemented/verified in new architecture
- ✅ **ACTIVE**: Currently used in frontend and backend implemented
- ❌ **REMOVED**: Handler removed (website/live-edit features)
- 🔄 **IN PROGRESS**: Being implemented
- ⚠️ **DEPRECATED**: Marked for removal

---

## Cleanup Checklist

At the end of migration:

1. ✅ **Review this file** - Identify unused functions
2. ⬜ **Remove unused handlers** from `tauri_handlers/src/handlers/`
3. ⬜ **Remove unused actions** from `helper.ts` handlerMap
4. ⬜ **Remove unused types** from `types/common.rs`
5. ⬜ **Update `tauri::generate_handler![]`** to remove unused handlers
6. ⬜ **Run cargo check** to verify no compilation errors
7. ⬜ **Test frontend** to ensure nothing breaks

---

## How to Update This File

When implementing a handler:
1. Change status from ⏳ TODO to 🔄 IN PROGRESS
2. Add "Used In" column entry (e.g., "FolderList.svelte")
3. When complete, change to ✅ ACTIVE
4. Add any notes about implementation details

When removing a handler:
1. Change status to ❌ REMOVED
2. Remove from handlerMap in helper.ts
3. Add note about why removed

---

## Complete Function Flows

### addFolder Flow

**Frontend → Handler → Service → Crypto Utils → Repository**

**Status**: ✅ Updated (2025-11-09) - Template-based permissions

1. **Frontend**: `sendMessage("addFolder", { name, description, folderTemplateJson })`
   - Location: `FolderManager.svelte:26`, `data.svelte.ts:169`
   - Calls: `invoke("handle_add_folder", { input: data })`
   - Types: `AddFolderInput` with folderTemplateJson (helper.ts:7)
   - **Changed**: Added `folderTemplateJson: JSON.stringify(FOLDER_TEMPLATE)`

2. **Handler**: `handle_add_folder`
   - Location: `tauri_handlers/src/handlers/folder.rs:19`
   - Returns: `BaseCryptoResponse::FolderCreated(folder)` → `FolderResponse` (helper.ts:12)

3. **Service**: `create_folder`
   - Location: `services/src/folder_service.rs:14`
   - **Template-Based Implementation** (2025-11-09):
   - Steps:
     - Validates folder name is not empty (line 24)
     - Creates new `Folder` model (line 29)
     - Gets encrypted UCAN key from store (line 32)
     - **Calls crypto_utils**: `generate_folder_ucan_with_template()` (line 35)
       - Location: `crypto_utils/src/ucan_utils.rs:718`
       - Parses folder_template_json from frontend
       - Extracts owner_template capabilities
       - Builds UCAN with capabilities from template
       - Embeds owner_template and node_template in facts
       - Returns: (folder_root_ucan_key, ucan_cid)
     - Creates `FolderShareRecord` for owner (line 47)
     - Saves folder and share record in transaction (line 55)

4. **Repository Functions**:
   - `store_repo.get_ucan_key()` - `persistance/src/repositories/store_repository.rs:233`
     - Fetches encrypted UCAN key from store_items table
   - `folder_repo.save_folder_with_share_record()` - `persistance/src/repositories/folder_repository.rs:23`
     - Inserts folder into `folders` table
     - Inserts share record into `folder_share_records` table
     - Both operations in single transaction

### getFolder Flow

**Frontend → Handler → Service → Repository**

1. **Frontend**: `sendMessage("getFolder")`
   - Location: `data.svelte.ts:119`
   - Calls: `invoke("handle_get_folders")`

2. **Handler**: `handle_get_folders`
   - Location: `tauri_handlers/src/handlers/folder.rs:42`
   - Returns: `BaseCryptoResponse::Folders(folder_responses)` → `FolderResponse[]` (helper.ts:12)

3. **Service**: `get_all_folders`
   - Location: `services/src/folder_service.rs:49`
   - Steps:
     - Calls repository to fetch all folders
     - Returns list of folders

4. **Repository Functions**:
   - `folder_repo.find_all()` - `persistance/src/repositories/folder_repository.rs`
     - Queries all non-deleted folders from `folders` table

### shareFolder Flow

**Frontend → Handler → Service → Crypto Utils → Repository**

**Status**: ✅ Updated (2025-11-09) - Template-based permissions

1. **Frontend**: `sendMessage("shareFolder", { folderId, userId, recipientRole })`
   - Location: `PublishWebsiteModal.svelte:119`
   - Calls: `invoke("handle_share_folder", { input: data })`
   - Types: `ShareFolder` with recipientRole: "node" (helper.ts)
   - **Changed**: Replaced `permissions` array with `recipientRole` string

2. **Handler**: `handle_share_folder`
   - Location: `tauri_handlers/src/handlers/folder.rs:85`
   - Returns: `BaseCryptoResponse::Success`

3. **Service**: `share_folder`
   - Location: `services/src/folder_service.rs:89`
   - **Template-Based Implementation** (2025-11-09):
   - Steps:
     - Validates folder exists (line 99)
     - Validates recipient user exists (line 106)
     - Checks if already shared using efficient single query (line 113)
       - Calls `folder_share_repo.find_by_folder_and_user()`
     - **Extracts template from owner's folder UCAN** (line 125-143):
       - Matches recipient_role ("owner" or "node") to template_key
       - Parses folder.ucan using `crypto_utils::ucan_utils::validate_structure()`
       - Gets facts from parsed UCAN
       - Extracts `owner_template` or `node_template` based on role
       - Gets capabilities map from template
     - Gets owner's encrypted UCAN key (line 153)
     - Decrypts UCAN signing keys (line 154)
     - **Calls crypto_utils**: `generate_flexible_folder_token()` (line 160)
       - Location: `crypto_utils/src/ucan_utils.rs`
       - Uses capabilities extracted from template
       - Recipient role: passed from frontend ("node")
       - Returns: folder_ucan_token
     - Generates CID from folder UCAN (line 174)
     - Creates and saves `FolderShareRecord` (line 177)
     - Gets all resources in folder (line 192)
     - **For each resource** (line 198):
       - Calls `resource_service::share_resource()` with same role
       - Passes `recipient_role` (not hardcoded "owner")
       - Location: `services/src/resource_service.rs`
       - Resource sharing uses same template-based delegation

4. **Repository Functions**:
   - `folder_share_repo.find_by_folder_and_user()` - `persistance/src/repositories/folder_share_repository.rs:165`
     - Single query to check if folder already shared
   - `resource_repo.find_all_by_folder()` - `persistance/src/repositories/resource_repository.rs`
     - Gets all resources in folder
   - `folder_share_repo.save()` - `persistance/src/repositories/folder_share_repository.rs:23`
     - Saves folder share record
   - `share_repo.save()` - `persistance/src/repositories/resource_share_repository.rs`
     - Saves resource share record

**Critical Bug Fix (2025-11-08) - UCAN Facts Missing in Delegated Tokens**:
- **Issue**: Delegated UCANs for nodes were missing the entire `fct` field
- **Owner UCAN had**: owner_template, viewer_template, doc_types, docs, role
- **Node's delegated UCAN had**: NO facts at all
- **Root Cause**: `generate_delegated_ucan()` only added capabilities and proof, not facts
- **Fix**: Updated `generate_delegated_ucan()` signature (ucan_utils.rs:1026)
  - Added parameters: template_value, recipient_role, doc_types_value, docs_list
  - Added facts to UCAN builder (lines 1067-1088)
- **Fix**: Updated `issue_flexible_delegated_resource_ucan()` (crypto_utils.rs:571)
  - Extracts facts from parent UCAN (lines 625-650)
  - Passes facts to `generate_delegated_ucan()` (lines 656-666)
- **Result**: Delegated UCANs now contain complete facts structure matching owner UCANs

**Key Architecture Points** (Updated 2025-11-09):
- **Template-based permissions**: Frontend defines all capabilities in `permissions.ts`
- **Role-based delegation**: Backend extracts appropriate template based on `recipient_role`
- **No hardcoded capabilities**: All permissions come from frontend configuration
- **Unified pattern**: Both folders and resources use template-based delegation
- UCANs and encrypted data generated **on-demand during sync**, not pre-created
- Share records only contain UCAN tokens (access control)
- Node will request encrypted data during sync when needed
- Facts propagate through delegation chain for CRDT/asset classification

**Functions Used** (Template-Based):
- **Crypto Utils**:
  - `generate_folder_ucan_with_template()` - NEW (replaces generate_folder_owner_ucan)
    - Location: `crypto_utils/src/ucan_utils.rs:718`
    - Parameters: Takes folder_template_json from frontend
    - Returns: (ucan_token, ucan_cid) with templates embedded in facts
  - `validate_structure()` - Parse and validate UCAN token
    - Location: `crypto_utils/src/ucan_utils.rs:305`
    - Used for extracting templates from owner UCAN
  - ❌ `generate_folder_owner_ucan()` - REMOVED (replaced by template-based function)

- **Validation Service**:
  - `validate_peer_can_add_folder()` - Validate peer connection token
    - Location: `services/src/validation_service.rs:22`
    - Checks peer has `add_folder` capability in connection token
    - Added detailed logging for debugging

- **Folder Service**:
  - `accept_folder_from_peer()` - Receive folder from peer
    - Location: `services/src/folder_service.rs:272`
    - Validates peer connection token and folder UCAN
    - Added detailed logging for debugging

- **Network Layer**:
  - `handshake::process_exchange_message()` - FIXED: Store correct peer token
    - Location: `network/src/p2p/handshake.rs:168-172`
    - Now stores payload.ucan_token (token peer sent us)
    - Critical for folder validation to work
  - `folder_sync::handle_folder_data_sync()` - Receive folder
    - Location: `network/src/p2p/folder_sync.rs:106`
    - Added detailed logging for validation debugging

**Error Handling**:
- New error variant: `FolderServiceError::InvalidRole { role: String }`
  - Location: `services/src/errors.rs:176`
  - Returned when recipient_role is not "owner" or "node"
- New error variant: `FolderServiceError::UcanError(String)`
  - Location: `services/src/errors.rs:179`
  - Returned when UCAN parsing or template extraction fails

### addResource Flow

**Frontend → Handler → Service → Crypto Utils → Repository**

1. **Frontend**: `sendMessage("addCredential", { resourcePayload, folderId, resourceType, ucanTemplateJson, metadataJson })`
   - Location: `data.svelte.ts:232`
   - Calls: `invoke("handle_add_resource", { input: data })`
   - Types: `AddResourceInput` (helper.ts:20)
   - UCAN Template: Owner/viewer capabilities for 6 docs:
     - template_doc, content_doc, user_content_doc, collaborative_doc, submissions_doc, static_assets
     - doc_types: Specifies "static_assets" as "asset", others as "crdt"
   - Metadata: Unencrypted JSON with title, type, client_id, last_modified, search config

2. **Handler**: `handle_add_resource`
   - Location: `tauri_handlers/src/handlers/resource.rs:18`
   - Returns: `BaseCryptoResponse::ResourceCreated(ResourceMetadata)` → `ResourceMetadata` (helper.ts:28)
   - Includes: id, title, resourceType, folderId, lastModified, favourite, preview

3. **Service**: `create_resource`
   - Location: `services/src/resource_service.rs:114`
   - Steps:
     - Generates UUID for resource (line 128)
     - Parses metadata JSON (line 131)
     - **Calls crypto_utils**: `encrypt_data_for_user()` (line 135)
       - Location: `crypto_utils/src/crypto_utils.rs`
       - Encrypts resource payload with user's public key
       - Uses AES-256-GCM encryption
       - Returns: (encrypted_data, encrypted_key)
     - Gets encrypted UCAN key from store (line 139)
     - **Calls crypto_utils**: `generate_flexible_resource_owner_ucan()` (line 144)
       - Location: `crypto_utils/src/ucan_utils.rs:607`
       - Parses UCAN template from frontend
       - Builds capabilities for each doc (all 6 docs including static_assets)
       - Adds UCAN facts:
         - `owner_template`: Full template with capabilities and doc_types
         - `viewer_template`: Viewer permissions template
         - `role`: "owner" (marks this as owner UCAN)
         - `doc_types`: Asset vs CRDT classification
         - `docs`: List of all document names for parsing
       - Signs UCAN with owner's key
       - Returns: (ucan_token, ucan_cid)
     - Creates EncryptedResource model (line 162)
     - Saves to database via repository (line 174)
     - Creates owner's ShareRecord (line 187)
     - Returns decrypted Resource via `Resource::from_decrypted_data()` (line 212)

4. **Repository Functions**:
   - `resource_repo.save_encrypted()` - `persistance/src/repositories/resource_repository.rs`
     - Inserts encrypted resource into `resources` table
     - Fields: id, folder_id, encrypted_data, encrypted_key, ucan_token, metadata (unencrypted), timestamps
   - `resource_share_repo.save_share_record()` - `persistance/src/repositories/resource_share_repository.rs`
     - Inserts owner's share record into `resource_share_records` table
     - Links resource to owner user with full permissions

### getCredential Flow

**Frontend → Handler → Service → Crypto Utils → Repository**

1. **Frontend**: `sendMessage("getCredential", { resourceId })`
   - Location: `data.svelte.ts:326`
   - Calls: `invoke("handle_get_resource", { input: data })`
   - Types: `GetResourceInput` (helper.ts:35)

2. **Handler**: `handle_get_resource`
   - Location: `tauri_handlers/src/handlers/resource.rs:68`
   - Returns: `BaseCryptoResponse::SelectedResourceResponse(ResourceResponse)` → `ResourceResponse` (helper.ts:39)

3. **Service**: `get_resource_by_id_direct`
   - Location: `services/src/resource_service.rs:234`
   - Steps:
     - Fetches encrypted resource from database (line 242)
     - Creates ResourceWithKey struct (line 248)
     - **Calls crypto_utils**: `decrypt_resource()` via `decrypt_resources()` helper (line 254)
       - Location: `crypto_utils/src/crypto_utils.rs:119`
       - Decrypts AES key with PGP private key
       - Decrypts data with AES-256-GCM
       - Returns: decrypted JSON string
     - Parses JSON and creates Resource via `Resource::from_decrypted_data()` (line 77 in decrypt_resources)
     - Returns Resource with loaded Loro documents

4. **Repository Functions**:
   - `resource_repo.find_by_id()` - `persistance/src/repositories/resource_repository.rs:71`
     - Queries single resource from `resources` table by ID
     - Returns: EncryptedResource with encrypted_data, encrypted_key, ucan_token, metadata

### getAllResourcesMetadata Flow

**Frontend → Handler → Service → Repository**

1. **Frontend**: `sendMessage("getAllResourcesMetadata")`
   - Location: `data.svelte.ts:133`
   - Calls: `invoke("handle_get_all_resources_metadata")`
   - Returns: `ResourceMetadata[]` (helper.ts:28)

2. **Handler**: `handle_get_all_resources_metadata`
   - Location: `tauri_handlers/src/handlers/resource.rs:111`
   - Returns: `BaseCryptoResponse::ResourcesMetadata(Vec<ResourceMetadata>)` → `ResourceMetadata[]` (helper.ts:28)

3. **Service**: `get_all_resources_metadata`
   - Location: `services/src/resource_service.rs:358`
   - Steps:
     - Fetches all encrypted resources from database (line 365)
     - For each resource, gets share record with encrypted_key (line 369-377)
     - Returns Vec<(EncryptedResource, encrypted_key)>
     - **No decryption** - metadata field is unencrypted JSON

4. **Handler Processing** (tauri_handlers/src/handlers/resource.rs:126):
   - Parses metadata JSON from each EncryptedResource (line 130)
   - Extracts fields:
     - `title` from metadata.title (line 134)
     - `resource_type` from metadata.type (line 140)
     - `last_modified` from metadata.last_modified (line 146)
   - Creates ResourceMetadata with camelCase fields (line 151)
   - Returns Vec<ResourceMetadata>

5. **Repository Functions**:
   - `resource_repo.get_all_resources()` - `persistance/src/repositories/resource_repository.rs:102`
     - Queries all resources from `resources` table for user
   - `resource_share_repo.find_by_resource_and_user()` - `persistance/src/repositories/resource_share_repository.rs`
     - Gets share record with encrypted_key for each resource

6. **Frontend Processing** (data.svelte.ts:136):
   - Maps ResourceMetadata to local Resource type
   - Adds to this.resources array
   - Populates resource list UI

**Key Difference from Old Approach**:
- **Old**: Backend emitted multiple `resource-added` events (one per resource)
- **New**: Single API call returns all metadata in one batch
- **Benefits**: Faster, simpler state management, easier to debug

### updateCredential Flow

**Frontend → Handler → Service → Crypto Utils → Repository**

1. **Frontend**: `sendMessage("updateCredential", { id, data })`
   - Location: `data.svelte.ts:327` (background save on switch), `data.svelte.ts:441` (explicit save)
   - Calls: `invoke("handle_update_resource", { input: data })`
   - Types: `UpdateResourceInput` (helper.ts:118)
   - Data: JSON string with Loro document snapshots (template_doc, content_doc, etc.)

2. **Handler**: `handle_update_resource`
   - Location: `tauri_handlers/src/handlers/resource.rs:168`
   - Returns: `BaseCryptoResponse::ResourceUpdated(ResourceMetadata)` → `ResourceMetadata` (helper.ts:28)

3. **Service**: `update_resource`
   - Location: `services/src/resource_service.rs:279`
   - Steps:
     - Parses data JSON to extract title for logging (line 301)
     - **Calls crypto_utils**: `encrypt_data_for_user(data, user.public_key)` (line 313)
       - Location: `crypto_utils/src/data_encryption.rs:7`
       - **Generates NEW random AES key** (key rotation for forward secrecy)
       - Encrypts data with new AES key using AES-256-GCM
       - Encrypts AES key with user's PGP public key
       - Returns: (encrypted_data, encrypted_key)
     - Calls repository to update resource (line 318)

4. **Repository Functions**:
   - `resource_repo.update_resource(encrypted_data, encrypted_key, resource_id)` - `persistance/src/repositories/resource_repository.rs:120`
     - Updates THREE fields in resources table:
       - `encrypted_data` - New encrypted Loro snapshots
       - `encrypted_key` - New AES key (rotated)
       - `updated_at` - Current timestamp
     - Single SQL UPDATE transaction

5. **Handler Post-Processing** (tauri_handlers/src/handlers/resource.rs:189):
   - Fetches updated EncryptedResource from database
   - Parses metadata JSON to extract:
     - `title` from metadata.title (line 199)
     - `resource_type` from metadata.type (line 205)
     - `last_modified` from metadata.last_modified (line 211)
   - Creates ResourceMetadata with camelCase fields (line 217)
   - Returns ResourceMetadata with updated timestamp

6. **Frontend Processing** (data.svelte.ts:327, 441):
   - Background save: Fire-and-forget when switching resources
   - Explicit save: Called by `saveCurrentResource()` method
   - Updates local `lastModified` timestamp (line 326, 438)

**Key Security Feature - AES Key Rotation**:
- **Every update generates a NEW AES key** for forward secrecy
- Old encrypted data cannot be decrypted even if old key is compromised
- Simple implementation for single-user (only owner's key in database)
- **Phase 3 consideration**: When multi-user sharing is implemented, will need to re-encrypt new key for all users with access via P2P sync

**Bug Fix (2025-11-07) - Metadata Separation**:
- **Issue**: Frontend was including metadata fields (client_id, last_modified, title) in the update data payload
- **Problem**: Service encrypted ENTIRE payload → Parser expected only document arrays → Failed with "expected a sequence, got string"
- **Root Cause**: `Resource::from_decrypted_data()` expects `HashMap<String, Vec<Value>>` - all fields must be arrays
- **Fix**: Removed metadata fields from loroContent in `saveCurrentResource()` (data.svelte.ts:415-423)
- **Now**: Only sends Loro document snapshots: template_doc, content_doc, user_content_doc, collaborative_doc, submissions_doc, static_assets
- **Result**: Updates now work correctly, matches add_resource pattern (documents vs metadata separation)
- **Key Learning**: encrypted_data should only contain Loro document snapshots (arrays), metadata stays in separate unencrypted column

### startP2PListener Flow

**Frontend → Handler → p2p_init Module**

1. **Frontend**: `sendMessage("startP2PListener")`
   - Location: (Called after successful login)
   - Calls: `invoke("start_p2p_listener")`

2. **Handler**: `start_p2p_listener`
   - Location: `tauri_handlers/src/handlers/p2p.rs:16`
   - Gets user and device from UserState
   - Returns: `BaseCryptoResponse::Success`

3. **P2P Init Module**: `p2p_init::initialize_p2p()`
   - Location: `network/src/p2p/p2p_init.rs:178`
   - Calls 4 initialization functions:
     1. `set_current_user()` (line 22) - Store user context
     2. `set_current_device()` (line 31) - Store device context
     3. `ensure_initialized()` (line 41) - Bind Iroh endpoint
     4. `start_listening()` (line 100) - Accept incoming connections

4. **P2P Init Functions Used**:
   - `set_current_user()` - Sets user in P2PService
   - `set_current_device()` - Sets device in P2PService
   - `ensure_initialized()` - Binds Iroh endpoint to network
   - `start_listening()` - Spawns async task to accept connections
   - Internal: calls `P2PService::perform_handshake_and_create_peer()` for each incoming connection

**Architecture Note**:
- Init logic separated into `p2p_init.rs` module
- Core networking in `p2p_service.rs`
- Sync operations will be in `sync_service.rs` (Phase 3)

**Does NOT auto-connect**: No automatic connection to known peers (manual only)

### addSovereignNode Flow

**Frontend → Handler → Service → P2P Init**

1. **Frontend**: `sendMessage("addSovereignNode", connectionString)`
   - Location: (Manual action - user pastes connection string)
   - Calls: `invoke("handle_add_sovereign_node", { input: connectionString })`
   - Types: Connection string is base64-encoded JSON from Kunki

2. **Handler**: `handle_add_sovereign_node`
   - Location: `tauri_handlers/src/handlers/p2p.rs:58`
   - Returns: `BaseCryptoResponse::Success`

3. **Flow**:
   - **Step 1**: Decode base64 connection string (line 67)
     - Parse to `UserDetails` struct containing:
       - `user_public_key`: Node's PGP public key
       - `device_public_key`: Node's device public key
       - `username`: Node operator username
       - `ucan_token`: One-time UCAN with role='owner'
       - `ucan_pub_key`: Node's UCAN public key

   - **Step 2**: Save node to database (line 81)
     - Calls `add_known_user()` from `services/src/user_service.rs:13`
     - Creates User with `first_sync=false`, `owner=false`
     - Stores one-time token in `users.ucan_token`
     - Returns (user, device)

   - **Step 3**: Connect immediately (line 97)
     - Calls `p2p_service.connect_with_ticket(&device.id)`
     - Location: `network/src/p2p/p2p_service.rs:118`
     - Establishes Iroh P2P connection
     - Calls `perform_handshake_and_create_peer()` (line 188)

   - **Step 4**: Handshake exchange
     - **Owner initiates** (`initiate_handshake()` - handshake.rs:37)
       - Extracts role from one-time token: `PeerRole::Owner`
       - Determines reciprocal role: `issued_token_role = "node"`
       - Issues persistent token with role='node' to node
       - Sends `FirstConnectRequest` with both tokens

     - **Node processes** (`process_first_user_connection_request()` - handshake.rs:279)
       - Validates one-time token
       - Extracts role: `PeerRole::Owner`
       - Issues persistent token with role='owner' to owner
       - Saves owner to DB with first_sync=true
       - Sends `FirstConnectResponse`

     - **Owner completes** (`process_first_user_connection_handshake_response()` - handshake.rs:405)
       - Validates issued token from node
       - Extracts role: `PeerRole::Owner`
       - Saves node to DB with first_sync=true
       - Sets `ConnectionType::Owner` from peer role
       - Emits `P2PEvent::UserConnected`

4. **Result**:
   - Owner's DB: Stores node with token (role='owner'), first_sync=true
   - Node's DB: Stores owner with token (role='node'), first_sync=true
   - Connection established, ready for sync operations

5. **Key Implementation Details**:
   - **Reciprocal roles**: Each party stores token describing WHO they're connecting to
   - **Happy path only**: No error handling, no auto-retry on failure
   - **Immediate connection**: Direct await, no background spawn
   - **Role-specific events**: `P2PEvent::UserConnected` for owner connections
   - **Password masking**: Kunki CLI uses `rpassword` for invisible password input

**Connection String Generation (Kunki)**:
- Command: `kunki token -p <passphrase>` or `kunki start --print-token`
- Function: `handle_token()` - `kunki/src/main.rs:367`
- Generates one-time UCAN with role='owner'
- Base64-encodes JSON with user/device keys and token
- Output copied by user and pasted into owner app

**Security Note**:
- One-time tokens are replaced with persistent tokens after first handshake
- 30-year lifetime for persistent tokens
- Role-based connection type enforcement
- Separate tokens for each connection direction

### Folder Sync Flow (After shareFolder)

**Handler → Sync Handler → Folder Sync → Resource Sync → Service Layer**

**Overview**: After `share_folder()` completes (creates ACLs and share records), the handler automatically triggers P2P sync to send the folder and all resources to the node.

1. **Handler Trigger**: `handle_share_folder`
   - Location: `tauri_handlers/src/handlers/folder.rs:108`
   - After `share_folder()` succeeds, calls sync handler:
   ```rust
   network::p2p::sync_handler::send_folder(
       folder_id, recipient_user_id, user,
       repo_ctx, crypto_utils, p2p_service
   )
   ```

2. **Sync Handler**: `sync_handler::send_folder()`
   - Location: `network/src/p2p/sync_handler.rs:20`
   - **Fire-and-forget pattern**: Spawns async task (line 29)
   - Calls internal implementation: `send_folder_impl()` (line 48)
   - Errors logged, doesn't block handler response

3. **Sync Handler Implementation**: `send_folder_impl()`
   - Location: `network/src/p2p/sync_handler.rs:48`
   - Steps:
     - **Step 1**: Get recipient's devices (line 57)
       - Calls `device_repo.get_devices_by_user_id()`
       - Takes first device: `devices[0]`

     - **Step 2**: Get or establish connection (line 70)
       - Try: `p2p_service.get_connection_by_id(&device.id)`
       - If fails: `p2p_service.connect_with_ticket(&device.id)` (line 79)
       - **Critical**: Connection indexed by `device.id`
       - Waits for handshake to complete before returning

     - **Step 3**: Delegate to folder_sync (line 82)
       - Calls `folder_sync::send_folder_with_resources()`
       - Passes peer connection and all context

4. **Folder Sync**: `send_folder_with_resources()`
   - Location: `network/src/p2p/folder_sync.rs:18`
   - Steps:
     - **Step 1**: Send folder data (line 27)
       - Calls `send_folder_data()` (internal function)
       - Gets folder via `get_folder_by_id()` from services
       - Gets folder share record via `get_folder_share_record()` from services
       - Creates `FolderDataSync` message
       - Sends via `peer_conn.send_message(Message::FolderDataSync(data))`

     - **Step 2**: Send all resources (line 36)
       - Calls `resource_sync::send_all_resources_for_folder()`

5. **Resource Sync**: `send_all_resources_for_folder()`
   - Location: `network/src/p2p/resource_sync.rs:18`
   - **Bulk operation** - optimized for multiple resources
   - Steps:
     - **Step 1**: Get all resources in folder (line 32)
       - Calls `resource_repo.find_all_by_folder(folder_id, user_id)`
       - Returns `Vec<EncryptedResource>`

     - **Step 2**: Get all share records in bulk (line 52)
       - Calls `get_resource_share_records_for_folder()` from services
       - Single query for all share records
       - Returns `Vec<ShareRecord>` with UCAN tokens

     - **Step 3**: Get recipient user (line 72)
       - Calls `user_repo.get_user_by_id(recipient_user_id)`
       - Need `public_key` for PGP encryption (NOT ucan_pub_key!)

     - **Step 4**: Loop through resources (line 85)
       - For each resource:
         - Find matching share record (line 87)
         - **Call service**: `prepare_resource_for_peer()` (line 102)
         - Create `ResourceDataSync` message (line 125)
         - Send via `send_resource_data()` (line 131)
       - **Partial success allowed**: Errors logged, continues with other resources

6. **Service Layer**: `prepare_resource_for_peer()`
   - Location: `services/src/resource_service.rs:647`
   - **Critical function** - decrypts, filters, re-encrypts for peer
   - Steps:
     - **Step 1**: Fetch original encrypted resource (line 658)
       - Calls `resource_repo.find_by_id(resource_id)`
       - Returns EncryptedResource with owner's encryption

     - **Step 2**: Decrypt resource (line 668)
       - Calls `crypto_utils.decrypt_resource(encrypted_data, encrypted_key)`
       - Decrypts with owner's PGP private key
       - Returns JSON string with Loro snapshots

     - **Step 3**: Parse to Resource (line 682)
       - Calls `Resource::from_decrypted_data()`
       - Loads Loro documents from snapshots
       - Returns Resource with 6 docs (template, content, user_content, collaborative, submissions, static_assets)

     - **Step 4**: Filter documents (line 693)
       - **Critical**: `resource.filter_to_send(&original_encrypted.ucan_token, peer_ucan)`
       - Location: `core/src/models/resource.rs:169`
       - Compares owner UCAN vs peer UCAN capabilities
       - Filters out documents peer doesn't have access to
       - Returns filtered `HashMap<String, Vec<Value>>`

     - **Step 5**: Serialize filtered data (line 703)
       - Converts filtered HashMap to JSON string

     - **Step 6**: Re-encrypt for peer (line 710)
       - **Critical bug fix**: Use `recipient.public_key` (PGP), NOT `ucan_pub_key`!
       - Calls `encrypt_data_for_user(filtered_json, peer_public_key)`
       - Generates NEW random AES key for peer
       - Encrypts data with AES, encrypts key with peer's PGP public key
       - Returns: (new_encrypted_data, new_encrypted_key)

     - **Step 7**: Create peer's EncryptedResource (line 717)
       - Calls `original_encrypted.re_encrypt_for_recipient()`
       - Location: `core/src/models/resource.rs:54`
       - Returns new EncryptedResource with:
         - Same id, folder_id, metadata
         - Peer's encrypted_data and encrypted_key
         - Peer's UCAN token (from share record)

7. **Message Send**: `send_resource_data()`
   - Location: `network/src/p2p/resource_sync.rs:150`
   - Creates `ResourceDataSync` message containing:
     - `resource`: EncryptedResource (re-encrypted for peer)
     - `share_record`: ShareRecord (with peer's UCAN)
   - Sends via `peer_conn.send_message(Message::ResourceDataSync(data))`

8. **Node Side Reception**: `handle_resource_data_sync()`
   - Location: `network/src/p2p/resource_sync.rs:31` (currently disabled)
   - **TODO**: Re-enable after implementing:
     - `validate_ucan()` method in CryptoUtils
     - `save_resource_with_share_record()` in ResourceRepository
   - Will validate UCAN and save resource+share in transaction

9. **Node Side Folder Reception**: `handle_folder_data_sync()`
   - Location: `network/src/p2p/folder_sync.rs:105`
   - **Currently active** (partial implementation)
   - Calls `folder_repo.save_folder_with_share_record()`
   - **TODO**: Add UCAN validation when `validate_ucan()` is implemented

---

## Key Architecture Decisions

### 1. Fire-and-Forget Sync Pattern
- Handler triggers sync but doesn't wait for completion
- Sync happens in background async task
- Errors logged, doesn't block user interaction
- User gets immediate success response from `share_folder()`

### 2. Connection Management
- Connection indexed by `device.id` (NOT node_id)
- Auto-connect if no active connection exists
- `connect_with_ticket()` waits for handshake completion
- Returns ready-to-use PeerConnection

### 3. Bulk Operations
- Single query for all share records (not N+1)
- Loop through resources sequentially
- Partial success model: errors don't stop other resources

### 4. Resource Filtering
- **Owner UCAN** contains all document capabilities
- **Peer UCAN** contains filtered capabilities based on role
- `filter_to_send()` compares UCANs to determine which docs to send
- Only sends documents peer has access to

### 5. Encryption Model
- **Owner encryption**: Original resource encrypted with owner's PGP key
- **Peer encryption**: Resource re-encrypted with peer's PGP key
- **New AES key** generated for each peer (not shared)
- **PGP public key** used for encryption, NOT UCAN public key

### 6. Critical Bug Fixes

**Bug 1: Wrong Public Key Used for Encryption (2025-11-08)**
- **Issue**: Using `recipient.ucan_pub_key` instead of `recipient.public_key`
- **Error**: "Failed to parse certificate: unexpected EOF"
- **Root Cause**: UCAN key is for authorization, PGP key is for encryption
- **Fix**: Changed line 106 in `resource_sync.rs` to use `recipient.public_key`
- **User Fields**:
  - `public_key`: PGP public key for encryption/decryption
  - `ucan_pub_key`: EdDSA public key for UCAN signing/verification

**Bug 2: UCAN Facts Propagation (2025-11-08)**
- Already documented in shareFolder flow above

### 7. Message Flow Summary

```
Owner App:
  ↓ shareFolder() creates ACLs
  ↓ handle_share_folder() triggers sync
  ↓ sync_handler::send_folder() spawns task
  ↓ send_folder_impl() gets/establishes connection
  ↓ folder_sync::send_folder_with_resources()
  ↓   └→ send_folder_data() → Message::FolderDataSync
  ↓   └→ resource_sync::send_all_resources_for_folder()
  ↓       └→ For each resource:
  ↓           └→ prepare_resource_for_peer() (decrypt, filter, re-encrypt)
  ↓           └→ send_resource_data() → Message::ResourceDataSync
  ↓
Network Layer (Iroh P2P)
  ↓
Node App:
  ↓ peer_connection::handle_messages()
  ↓ peer_connection::process_message()
  ↓   └→ Message::FolderDataSync → folder_sync::handle_folder_data_sync()
  ↓   └→ Message::ResourceDataSync → resource_sync::handle_resource_data_sync()
  ↓       └→ (TODO) validate_ucan() + save_resource_with_share_record()
```

### 8. Functions Used in Sync Flow

**Network Layer** (network/src/p2p/):
- `sync_handler::send_folder()` - Entry point, spawns async task
- `sync_handler::send_folder_impl()` - Gets connection, delegates to folder_sync
- `folder_sync::send_folder_with_resources()` - Sends folder then resources
- `folder_sync::send_folder_data()` - Sends folder and share record
- `folder_sync::handle_folder_data_sync()` - Receives folder on node side
- `resource_sync::send_all_resources_for_folder()` - Bulk resource send
- `resource_sync::send_resource_data()` - Single resource send
- `resource_sync::handle_resource_data_sync()` - Receives resource (TODO)
- `peer_connection::send_message()` - Low-level message send

**Service Layer** (services/src/):
- `folder_service::get_folder_by_id()` - Fetch folder for sync
- `share_service::get_folder_share_record()` - Fetch folder share record
- `share_service::get_resource_share_records_for_folder()` - Bulk fetch share records
- `resource_service::prepare_resource_for_peer()` - Decrypt, filter, re-encrypt

**Core Models** (core/src/models/):
- `Resource::from_decrypted_data()` - Parse JSON to Resource with Loro docs
- `Resource::filter_to_send()` - Filter docs based on UCAN capabilities
- `EncryptedResource::re_encrypt_for_recipient()` - Create peer's encrypted resource

**Crypto Utils** (crypto_utils/src/):
- `decrypt_resource()` - Decrypt with owner's PGP key
- `encrypt_data_for_user()` - Encrypt with peer's PGP key
- (TODO) `validate_ucan()` - Validate UCAN token on receive

**Repository Layer** (persistance/src/repositories/):
- `device_repo.get_devices_by_user_id()` - Get recipient's devices
- `user_repo.get_user_by_id()` - Get recipient user
- `resource_repo.find_all_by_folder()` - Get all resources in folder
- `resource_repo.find_by_id()` - Get single encrypted resource
- `folder_repo.save_folder_with_share_record()` - Save folder on node
- (TODO) `resource_repo.save_resource_with_share_record()` - Save resource on node

---

## Resource Sync Validation Flow (2025-11-09)

### Functions Used

**Validation Service** (services/src/validation_service.rs):
- `validate_peer_can_add_resources()` - Validates owner's folder UCAN for add_resources capability
  - Location: services/src/validation_service.rs:64-95
  - Purpose: Validate owner has permission to add resources to specific folder
  - Steps:
    1. Validate folder UCAN structure
    2. Extract folder_id with add_resources capability
    3. Verify folder_id matches resource.folder_id

**Crypto Utils UCAN Functions** (crypto_utils/src/ucan_utils.rs):
- `extract_folder_id_with_add_resources_capability()` - Extract folder_id from UCAN with capability check
  - Location: crypto_utils/src/ucan_utils.rs:466-497
  - Purpose: Find folder UCAN capability and extract folder_id
  - Returns: folder_id if found with add_resources ability
  - Error: UcanError::CapabilityNotFound if not found

**Resource Service** (services/src/resource_service.rs):
- `accept_resource_from_peer()` - Accept and save resource from peer
  - Location: services/src/resource_service.rs:748-788
  - Purpose: Validate and save resource + share records from peer
  - Validation: Calls validate_peer_can_add_resources()
  - Save: Calls resource_repo.save_resource_with_share_records()

**Repository Layer** (persistance/src/repositories/resource_repository.rs):
- `save_resource_with_share_records()` - Save resource + share records in transaction
  - Location: persistance/src/repositories/resource_repository.rs:204-247
  - Purpose: Atomic save of resource with all share records
  - Transaction: Uses diesel transaction for atomicity
  - Strategy: insert_or_ignore_into for idempotent saves

**Network Layer** (network/src/p2p/):
- `handshake::process_exchange_message()` - Fixed peer token storage
  - Location: network/src/p2p/handshake.rs:168-172
  - Fix: Store payload.ucan_token (token peer sent us) instead of peer_user.ucan_token
  - Impact: Both folder_sync and resource_sync now get correct peer connection token

- `resource_sync::handle_resource_data_sync()` - Receive resource from peer
  - Location: network/src/p2p/resource_sync.rs:157-187
  - Purpose: Orchestrate resource reception, delegate to service
  - Delegates: Calls services::accept_resource_from_peer()

### Key Learnings from This Session

1. **Connection vs Folder Capabilities**:
   - Connection tokens have `{domain}:add_folder` (connection-level)
   - Folder UCANs have `{domain}:folder:{folder_id}:add_resources` (folder-level)
   - NEVER validate folder-level permissions with connection tokens!

2. **Folder UCAN Usage**:
   - Owner sends their folder UCAN (folder.ucan field) with each resource
   - Proves owner has add_resources permission for that specific folder
   - Node validates folder_id in UCAN matches resource.folder_id

3. **Handshake Token Storage**:
   - Must store the token peer SENDS us (proves their capabilities)
   - NOT the token we ISSUED to them (just records what we gave them)
   - Fixed in process_exchange_message() by using payload.ucan_token

4. **Share Records in ResourceDataSync**:
   - Send ALL share records (Vec<ShareRecord>), not just one
   - Enables node to forward viewer updates back to owner
   - Node needs to know all viewers who have access

5. **Transaction-Based Saving**:
   - Resource + share records saved in single transaction
   - Uses insert_or_ignore_into for idempotent saves (re-sending is safe)
   - Atomicity ensures consistency

---

## Resource Sync (CRDT Merge Protocol)

**Status**: ✅ Implemented (2025-11-09)
**Purpose**: Bidirectional CRDT synchronization between peers using state vectors

### Flow

```
User clicks sync → syncResource → handle_sync_resource →
network::p2p::sync_handler::sync_resource →
(For each peer with access):
  - Get share records → services::get_all_share_records_for_resource()
  - Get UCANs → services::get_resource_ucans_for_sync()
  - Send ResourceSyncRequest(resource_ucan, folder_ucan)

Peer receives → handle_resource_sync_request →
  - Check if resource exists
  - If exists: Get state vectors → services::get_resource_state_vectors_by_ucan()
  - Send StateVectorRequest(state_vectors, ucan_token)

Initiator receives → handle_state_vector_request →
  - Generate updates → services::generate_updates_for_peer()
  - Send UpdatesResponse(updates, state_vectors, ucan_token)

Peer receives → handle_updates_response →
  - Apply updates → services::apply_peer_updates()
  - (TODO: Send our updates back)
```

### Key Functions

**Tauri Handler** (tauri_handlers/src/handlers/resource.rs):
- `handle_sync_resource()` - Entry point for resource sync
  - Location: tauri_handlers/src/handlers/resource.rs:242-271
  - Purpose: Initiate sync from frontend
  - Pattern: Fire-and-forget (spawns async task)
  - Call: `network::p2p::sync_handler::sync_resource()`

**Sync Orchestration** (network/src/p2p/sync_handler.rs):
- `sync_resource()` - Orchestrate resource sync with all peers
  - Location: network/src/p2p/sync_handler.rs:125-241
  - Purpose: Find peers, get UCANs, send sync requests
  - Pattern: For each peer with access (skip self)
  - Calls:
    - `services::get_all_share_records_for_resource()` - Find users with access
    - `services::get_resource_ucans_for_sync()` - Get resource + folder UCANs
    - `p2p_service.get_connection_by_id()` or `connect_with_ticket()` - Get connection
    - `peer_conn.send_message(Message::ResourceSyncRequest)` - Send request

**Network Handlers** (network/src/p2p/resource_sync.rs):
- `handle_resource_sync_request()` - Receive sync request
  - Location: network/src/p2p/resource_sync.rs:400-492
  - Purpose: Check if resource exists, initiate CRDT merge
  - If resource exists: Send StateVectorRequest
  - If not: Send ResourceNotFoundRequest (not implemented)
  - Calls: `services::get_resource_state_vectors_by_ucan()`

- `handle_state_vector_request()` - Receive state vectors, send updates
  - Location: network/src/p2p/resource_sync.rs:555-608
  - Purpose: Generate incremental updates based on peer's state
  - Calls: `services::generate_updates_for_peer()`
  - Sends: UpdatesResponse with incremental Loro updates

- `handle_updates_response()` - Receive and apply updates
  - Location: network/src/p2p/resource_sync.rs:632-680
  - Purpose: Apply peer's updates, generate our updates back
  - Calls: `services::apply_peer_updates()`
  - TODO: Send our updates back to complete bidirectional sync

**Service Layer** (services/src/resource_service.rs):
- `get_resource_ucans_for_sync()` - Get resource + folder UCANs
  - Location: services/src/resource_service.rs:763-813
  - Purpose: Get UCANs needed for ResourceSyncRequest
  - Finds: Share record with operation="share" for user
  - Returns: (resource_ucan, folder_ucan)

- `get_resource_state_vectors_by_ucan()` - Get UCAN-filtered state vectors
  - Location: services/src/resource_service.rs:463-517
  - Purpose: Get state vectors only for docs peer can access
  - UCAN Filtering: Only include docs in UCAN capabilities
  - Returns: JSON with state vectors per doc

- `generate_updates_for_peer()` - Generate incremental updates
  - Location: services/src/resource_service.rs:519-580
  - Purpose: Generate Loro updates based on peer's state vectors
  - UCAN Filtering: Only send updates for permitted docs
  - Incremental: Uses `document::export_updates(doc, &peer_state_vector)`
  - Returns: JSON with updates + current state per doc

- `apply_peer_updates()` - Apply updates and generate response
  - Location: services/src/resource_service.rs:582-645
  - Purpose: Merge peer's updates, generate our updates back
  - Validation: Checks peer's UCAN for each doc
  - Rejects: Updates for `crud/readonly` docs
  - Bidirectional: Applies their updates AND generates our updates
  - Saves: Merged resource to database
  - Returns: JSON with our updates for peer

- `get_all_share_records_for_resource()` - Get all users with access
  - Location: services/src/share_service.rs (new file)
  - Purpose: Find all peers to sync with
  - Returns: Vec<ShareRecord> for resource

**Helper Functions** (tauri_handlers/src/types/common.rs):
- `SyncResourceInput` - Input type for sync command
  - Location: tauri_handlers/src/types/common.rs:255-259
  - Fields: resource_id

**Frontend Integration** (sthalam/frontend/desktop/src/utils/helper.ts):
- `syncResource` action - Call sync command
  - Location: helper.ts:107
  - Handler map: `invoke("handle_sync_resource", { input: data })`
  - Usage: `sendMessage("syncResource", { resourceId: "abc123" })`

### Message Flow

1. **ResourceSyncRequest** (`Message::ResourceSyncRequest`):
   - Contains: resource_ucan, folder_ucan
   - Purpose: Initiate sync for a resource
   - Response: StateVectorRequest (if resource exists)

2. **StateVectorRequest** (`Message::MergeUpdate(StateVectorRequest)`):
   - Contains: resource_id, state_vectors (JSON), asset_ids, ucan_token
   - Purpose: "Here are my state vectors, send me your updates"
   - Response: UpdatesResponse

3. **UpdatesResponse** (`Message::MergeUpdate(UpdatesResponse)`):
   - Contains: resource_id, updates (JSON), state_vectors, missing_asset_ids, ucan_token
   - Purpose: "Here are updates you're missing + my current state"
   - Response: None (sync complete) or UpdatesResponse (bidirectional)

### State Vector Format

```json
{
  "template_doc": {
    "state_vector": [1, 229, 249, 220, 160, ...]
  },
  "content_doc": {
    "state_vector": [1, 152, 156, 175, 191, ...]
  },
  "collaborative_doc": {
    "state_vector": [0]
  }
}
```

- Empty `[0]`: No operations yet
- Non-empty: Loro's version vector encoding
- **Filtered**: Only includes docs peer has permission for

### Updates Format

```json
{
  "content_doc": {
    "state_vector": [1, 152, 156, 175, 191, ...],
    "updates": [108, 111, 114, 111, 0, 0, ...]
  }
}
```

- `state_vector`: Sender's current state
- `updates`: Incremental Loro operations

### Key Learnings from Resource Sync

1. **Operation Type = "share"**:
   - All share records currently have operation_type = "share"
   - `get_resource_ucans_for_sync()` queries for operation="share"
   - Fixed from looking for "view" which didn't exist

2. **State Vector Sync Protocol**:
   - Initiator sends ResourceSyncRequest with UCANs
   - Peer responds with StateVectorRequest (their state)
   - Initiator sends UpdatesResponse (incremental updates)
   - Peer applies updates and generates response
   - Bidirectional: Both sides converge to same state

3. **UCAN Filtering Throughout**:
   - State vectors: Only include permitted docs
   - Updates generation: Only send permitted updates
   - Update application: Validate permissions for each doc
   - Security: Prevents unauthorized document access

4. **Incremental Sync**:
   - State vectors enable minimal data transfer
   - Only send operations peer doesn't have
   - Loro handles efficient binary encoding
   - Much better than full snapshot sync

5. **P2PService State Management**:
   - p2p_service managed as `Arc<P2PService>` in lib.rs
   - NOT wrapped in Mutex like originally attempted
   - Handler uses `p2p_service.inner().clone()` directly
   - Matches folder sharing pattern

6. **Fire-and-Forget Pattern**:
   - Handler spawns async task for sync
   - Returns success immediately
   - User doesn't wait for sync completion
   - Errors logged, don't propagate

---

## Notes

- This file tracks **frontend → backend** function calls
- **crypto_utils functions** are tracked to show encryption and UCAN generation
- Repository functions are tracked to show database operations
- Auth functions are considered core and always kept
- Website-related handlers already removed (not needed for sthalam core)
- **Folder sync flow** added 2025-11-08 after implementing simple folder sync protocol
- **Resource sync validation** added 2025-11-09 after implementing folder UCAN validation
- **Resource sync (CRDT merge)** added 2025-11-09 after implementing bidirectional state vector sync
