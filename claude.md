# Sthalam - Osvauld BlockSuite Website Builder

## Tech Stack
- **Frontend**: Svelte 5 (runes) - `.svelte` files only
- **Backend**: Rust with Tauri
- **Editor**: BlockSuite-inspired infinite canvas
- **P2P**: libp2p for networking
- **State**: Yjs for collaborative editing

## Quick Start
```bash
cd sthalam/src-tauri
cargo tauri dev
```

---

## Current Status (2025-10-13)

### ✅ Completed
- Infinite canvas website builder with drag/drop blocks
- Block types: Text, Heading, Container, Image, HTML/CSS
- **Form blocks**: Form Container, Text/Email/Number inputs, Textarea, Checkbox, Submit Button
- Yjs-based data synchronization
- Builder mode (full editing)
- Viewer mode (readonly display)
- **Forms are interactive in viewer mode** - can submit with JSON output
- **Form and Website share same WebsiteBuilder** (block-based architecture)
- P2P connection with UCAN tokens
- Basic publishing and connecting with token
- Three separate resource types: Website, NoticeBoard, Form
- Resource type selection modal when creating new pages
- Resource type-specific document state keys in Rust
- **Unified dataState architecture** - single source of truth for resources + coordinator
- **NoticeBoard UI**: Reddit-style thread builder with rich text/HTML/CSS/markdown support
  - Main thread editor with markdown, HTML/CSS, and preview modes
  - Nested comment threading with collapse/expand
  - Comment input with markdown, HTML/CSS, and preview
  - Rich text rendering with syntax highlighting

### 🚧 In Progress - VIEWER INTERACTION IMPLEMENTATION
- **Split document architecture for threads** (thread_doc + thread_comments_doc)
- **Resource-specific UCAN token generation** (different capabilities per resource type)
- **Backend authorization and validation** (authorship for threads, append-only for forms)
- **Forms with separate submissions storage** (form-{id}-submissions private resource)
- **Dedicated form submission message flow** (FormMessage, not Yjs sync)
- **Viewer mode UI for threads and forms**

---

## IMPLEMENTATION PLAN: Viewer Interactions & Permissions (2025-10-13)

This section details the comprehensive architecture for implementing viewer interactions with proper permissions, authorization, and data separation.

---

## 1. ARCHITECTURE OVERVIEW

### Core Principles
1. **Resource-Specific Permissions**: Each resource type has different viewer capabilities
2. **Split Document Architecture**: Separate read-only content from viewer-writable content
3. **Token-Based Authorization**: UCAN tokens express granular capabilities per resource
4. **Sovereign Node Mediation**: All viewer updates go through owner's sovereign node
5. **CRDT-Based Merging**: Use Yjs for conflict-free collaborative editing

### Permission Matrix
| Resource Type | Viewer Can Read | Viewer Can Write | Storage Model |
|---------------|-----------------|------------------|---------------|
| **Website**   | blocksuite_doc  | ❌ Nothing       | Single doc    |
| **Thread**    | thread_doc      | thread_comments_doc | Split docs |
| **Form**      | form_doc        | Separate submissions resource | Two resources |

---

## 2. THREAD ARCHITECTURE (Split Documents)

### Current Structure (Single Doc) ❌
```typescript
// Current implementation - TO BE CHANGED
thread_doc (Y.Doc):
  └─ blocks (Y.Map):
      ├─ thread-post-1 (type: thread-post, owner creates)
      ├─ comment-1 (type: comment, viewer adds)
      └─ comment-2 (type: comment, viewer adds)
```

### New Structure (Split Docs) ✅
```typescript
// NEW implementation
Resource: "thread-xyz"
├─ thread_doc (Y.Doc - READ-ONLY for viewers):
│   └─ blocks (Y.Map):
│       └─ thread-post-1 {
│            id: "thread-post-1",
│            type: "thread-post",
│            content: "# Main Post...",
│            mode: "markdown" | "html",
│            css: "/* custom styles */",
│            author: "owner-id",
│            timestamp: "2025-10-13T...",
│            order: 0
│          }
│
└─ thread_comments_doc (Y.Doc - READ-WRITE for viewers):
    └─ blocks (Y.Map):
        ├─ comment-1 {
        │    id: "comment-1",
        │    type: "comment",
        │    content: "Great post!",
        │    mode: "markdown",
        │    css: "",
        │    author: "viewer-123",
        │    timestamp: "2025-10-13T...",
        │    parentId: null,  // Top-level comment
        │    order: 1
        │  }
        └─ comment-2 {
             id: "comment-2",
             type: "comment",
             content: "I agree",
             mode: "markdown",
             css: "",
             author: "viewer-456",
             timestamp: "2025-10-13T...",
             parentId: "comment-1",  // Reply to comment-1
             order: 2
           }
```

### Why Split Documents?
1. **Permission Separation**: Owner can modify post, viewers cannot
2. **Easier Authorization**: Check which doc is being updated
3. **Clean UCAN Capabilities**: Different capabilities for different docs
4. **Viewer Comment Editing**: Viewers can edit their own comments without touching the main post

### Storage in Backend
```json
// Resource data structure
{
  "thread_doc": [/* Yjs update bytes - main post */],
  "thread_comments_doc": [/* Yjs update bytes - all comments */],
  "client_id": "123456",
  "last_modified": 1697123456789,
  "title": "Discussion Thread"
}
```

### UCAN Token Capabilities for Threads
```rust
// Token issued to viewer for thread resource
{
  "capabilities": [
    {
      "resource": "sthalam:resource:thread-xyz:thread_doc",
      "action": "crud/read"  // Can only read the main post
    },
    {
      "resource": "sthalam:resource:thread-xyz:thread_comments_doc",
      "action": "crud/write",  // Can add/edit comments
      "constraints": {
        "author_validation": true  // Must only edit own comments
      }
    }
  ],
  "subject": "viewer-user-id"  // Extracted for authorship validation
}
```

---

## 3. FORMS ARCHITECTURE (Separate Resources)

### Resource Structure
```
PUBLIC Resource (viewers can access):
├─ resource_id: "form-abc123"
├─ resource_type: "form"
├─ folder_id: "public-folder-1"
└─ form_doc (Y.Doc):
    └─ blocks (Y.Map):
        ├─ form-config block (field definitions)
        ├─ form-field-text blocks
        ├─ form-field-email blocks
        └─ form-submit-button block

PRIVATE Resource (owner + node only):
├─ resource_id: "form-abc123-submissions"
├─ resource_type: "form_submissions"
├─ folder_id: "private-submissions-folder"
└─ form_submissions_doc (Y.Doc):
    └─ blocks (Y.Map):
        ├─ submission-1 {
        │    id: "sub-001",
        │    type: "form-submission",
        │    viewer_id: "viewer-123",
        │    timestamp: 1697123456789,
        │    form_data: {
        │      name: "John Doe",
        │      email: "john@example.com",
        │      message: "Hello world"
        │    }
        │  }
        ├─ submission-2 { ... }
        └─ submission-3 { ... }
```

### Why Separate Submissions Resource?
1. **Privacy**: Viewers cannot see other viewers' submissions
2. **Access Control**: Only owner has access to submissions resource
3. **Independent Sync**: Form definition and submissions sync separately
4. **Scalability**: Thousands of submissions don't bloat form doc

### UCAN Token Capabilities for Forms
```rust
// Binding token issued to viewer for form
{
  "capabilities": [
    {
      "resource": "sthalam:resource:form-abc123:form_doc",
      "action": "crud/read"  // Can read form definition
    },
    {
      "resource": "sthalam:resource:form-abc123-submissions:form_submissions_doc",
      "action": "crud/append",  // Can ONLY append submissions
      "constraints": {
        "bound_to_form": "form-abc123",  // Only for this form
        "merge_target": "form-abc123-submissions",  // Where to merge
        "append_only": true  // Cannot modify existing submissions
      }
    }
  ],
  "subject": "viewer-user-id"
}
```

### Submission Flow (NEW - Dedicated Message, Not Yjs Sync)
```
1. Viewer fills out form
2. Viewer clicks Submit
3. Frontend creates FormSubmission object:
   {
     id: "sub-" + timestamp,
     type: "form-submission",
     viewer_id: from_user_context,
     timestamp: Date.now(),
     form_data: { field1: "value1", ... }
   }
4. Frontend sends FormMessage::SubmitFormData to node via IPC:
   {
     form_id: "form-abc123",
     submission: FormSubmission object,
     ucan_token: viewer_token
   }
5. Backend validates:
   - Token has "crud/append" permission for "{form_id}-submissions" resource
   - Token is valid and not expired
6. Backend appends submission to private submissions resource (append_form_submission)
7. Backend responds with FormMessage::SubmitFormDataResponse:
   {
     success: true/false,
     submission_id: "sub-123" or None,
     error: error_message or None
   }
8. Owner can load submissions resource from database → Views in dashboard

NOTE: This is NOT using Yjs sync mechanism - it's a dedicated one-way operation
```

### Why Separate Blocks for Each Submission?
**Pros:**
- ✅ CRDT merge works perfectly (no conflicts between concurrent submissions)
- ✅ Easy querying (iterate blocks, filter by viewer_id)
- ✅ Scalable (can handle millions of submissions)
- ✅ Append-only validation is simple (check if block is new)
- ✅ Consistent with threads (comments also use blocks)

**Cons:**
- ❌ More Yjs overhead (each submission is a Y.Map entry)

**Decision:** Use separate blocks (pros outweigh cons)

---

## 4. BACKEND IMPLEMENTATION

### 4.1 Token Generation (`node_service.rs`)

**Current:**
```rust
// prepare_resource_for_viewer() issues same token for all types
let permissions = vec![
    (format!("sthalam:resource:{}", resource_id), "crud/read".to_string()),
    (format!("sthalam:resource:{}", resource_id), "crud/update".to_string()),
];
```

**NEW - Resource-Specific Tokens:**
```rust
pub async fn prepare_resource_for_viewer(
    resource_id: &str,
    viewer_user: &User,
    local_user: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<ResourceSyncData> {
    // ... existing code to get resource ...

    // NEW: Determine permissions based on resource type
    let permissions = match resource_with_key.resource.resource_type.as_str() {
        "website" => vec![
            (
                format!("sthalam:resource:{}:blocksuite_doc", resource_id),
                "crud/read".to_string()
            ),
        ],

        "noticeboard" => vec![
            (
                format!("sthalam:resource:{}:thread_doc", resource_id),
                "crud/read".to_string()
            ),
            (
                format!("sthalam:resource:{}:thread_comments_doc", resource_id),
                "crud/write".to_string()
            ),
        ],

        "form" => {
            // Create submissions resource if doesn't exist
            let submissions_resource_id = format!("{}-submissions", resource_id);
            create_or_get_submissions_resource(
                &submissions_resource_id,
                resource_id,
                local_user,
                repo_ctx.clone(),
                crypto_utils
            ).await?;

            vec![
                (
                    format!("sthalam:resource:{}:form_doc", resource_id),
                    "crud/read".to_string()
                ),
                (
                    format!("sthalam:resource:{}:form_submissions_doc", submissions_resource_id),
                    "crud/append".to_string()
                ),
            ]
        },

        _ => vec![
            (
                format!("sthalam:resource:{}", resource_id),
                "crud/read".to_string()
            ),
        ]
    };

    // Issue UCAN with these capabilities
    let (ucan_token, ucan_cid) = {
        let crypto = crypto_utils.read().await;
        crypto.issue_delegated_resource_ucan(
            &encrypted_ucan_pvt_key,
            &local_share_record.ucan_token,
            &resource_owner.ucan_pub_key,
            resource_id,
            &viewer_user.ucan_pub_key,
            permissions,  // Resource-specific permissions
            &proof_resolver,
        ).await?
    };

    // ... rest of function ...
}

/// NEW FUNCTION: Create submissions resource for forms
async fn create_or_get_submissions_resource(
    submissions_resource_id: &str,
    form_resource_id: &str,
    owner: &User,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<String> {
    // Check if submissions resource already exists
    if let Ok(_) = repo_ctx.resource_repo.find_by_id(submissions_resource_id, &owner.id).await {
        return Ok(submissions_resource_id.to_string());
    }

    // Create private submissions folder if doesn't exist
    let private_folder_id = format!("{}-submissions-folder", form_resource_id);
    // ... create folder logic ...

    // Create empty Yjs doc for submissions
    let empty_doc = create_empty_submissions_doc();

    // Create resource
    let resource = create_resource(
        serde_json::to_string(&empty_doc)?,
        "form_submissions".to_string(),
        private_folder_id,
        owner,
        &owner.id,
        "sthalam",
        repo_ctx.clone(),
        crypto_utils,
    ).await?;

    Ok(resource.id)
}
```

### 4.2 Authorization Validation (`resource_service.rs`)

**Current:**
```rust
pub async fn apply_updates(
    resource_id: &str,
    updates: &str,
    user_id: &str,
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<()> {
    let (mut decrypted_resource, encrypted_key) =
        get_resource(resource_id, repo_ctx.clone(), user_id, crypto_utils).await?;

    // Directly apply updates - NO VALIDATION ❌
    decrypted_resource.sync_updates(updates).await?;

    // ... save ...
}
```

**NEW - With Authorization:**
```rust
pub async fn apply_updates(
    resource_id: &str,
    updates: &str,
    user_id: &str,
    author_id: &str,  // NEW: Extract from UCAN token subject
    repo_ctx: Arc<RepositoryContext>,
    crypto_utils: &Arc<RwLock<CryptoUtils>>,
) -> ServiceResult<()> {
    let (mut decrypted_resource, encrypted_key) =
        get_resource(resource_id, repo_ctx.clone(), user_id, crypto_utils).await?;

    // NEW: Validate based on resource type
    match decrypted_resource.resource_type.as_str() {
        "noticeboard" => {
            validate_thread_comment_authorship(
                &decrypted_resource,
                updates,
                author_id
            ).await?;
        },
        "form_submissions" => {
            validate_form_submission_append_only(
                &decrypted_resource,
                updates,
                author_id
            ).await?;
        },
        _ => {
            // Website and other types: no special validation
        }
    }

    // Apply updates
    decrypted_resource.sync_updates(updates).await?;

    // ... save ...
}

/// NEW FUNCTION: Validate thread comment authorship
async fn validate_thread_comment_authorship(
    resource: &DecryptedResource,
    updates: &str,
    author_id: &str
) -> ServiceResult<()> {
    // 1. Parse Yjs update to extract modified block IDs
    let modified_blocks = parse_yjs_update_blocks(updates)?;

    // 2. Load thread_comments_doc from resource.data
    let comments_doc_data = resource.data
        .get("thread_comments_doc")
        .ok_or(ResourceServiceError::ParseError("Missing thread_comments_doc".to_string()))?;

    let mut doc = Y::Doc::new();
    Y::applyUpdateV2(&mut doc, comments_doc_data.as_array())?;
    let blocks_map = doc.getMap("blocks");

    // 3. For each modified block, validate authorship
    for block_id in modified_blocks {
        if let Some(existing_block) = blocks_map.get(&block_id) {
            // Block exists - check if author matches
            let block_author = existing_block
                .get("author")
                .and_then(|v| v.as_str())
                .ok_or(ResourceServiceError::Unauthorized("Missing author field".to_string()))?;

            if block_author != author_id {
                return Err(ResourceServiceError::Unauthorized(
                    format!("Cannot edit comment by another user: {}", block_id)
                ));
            }
        } else {
            // New block - allow (append operation)
            // Backend will set author field from token
            continue;
        }
    }

    Ok(())
}

/// NEW FUNCTION: Validate form submission is append-only
async fn validate_form_submission_append_only(
    resource: &DecryptedResource,
    updates: &str,
    author_id: &str
) -> ServiceResult<()> {
    // 1. Parse Yjs update
    let modified_blocks = parse_yjs_update_blocks(updates)?;

    // 2. Load existing submissions doc
    let submissions_doc_data = resource.data
        .get("form_submissions_doc")
        .ok_or(ResourceServiceError::ParseError("Missing form_submissions_doc".to_string()))?;

    let mut doc = Y::Doc::new();
    Y::applyUpdateV2(&mut doc, submissions_doc_data.as_array())?;
    let blocks_map = doc.getMap("blocks");

    // 3. Check all blocks are NEW (append-only)
    for block_id in modified_blocks {
        if blocks_map.contains(&block_id) {
            // Block already exists - NOT ALLOWED
            return Err(ResourceServiceError::Unauthorized(
                format!("Cannot modify existing submission: {}", block_id)
            ));
        }
    }

    Ok(())
}

/// NEW HELPER: Parse Yjs update to extract modified block IDs
fn parse_yjs_update_blocks(updates: &str) -> ServiceResult<Vec<String>> {
    // Decode base64 updates string
    let update_bytes = base64::decode(updates)?;

    // Parse Yjs update structure
    // This requires understanding Yjs binary format
    // For POC, we can use a simplified approach:
    // - Decode update
    // - Apply to temporary doc
    // - Compare before/after to find changed blocks

    // TODO: Implement proper Yjs update parsing
    // For now, return empty (disable validation for POC)
    Ok(vec![])
}
```

### 4.3 Update Message Flow (`resource_sync.rs`)

**Modify `process_resource_update_message` to extract author_id:**

```rust
pub async fn process_resource_update_message(
    &self,
    payload: &ResourceUpdateMsg,
) -> P2PResult<()> {
    let user = self.get_local_user().await?;
    let peer_user = self.get_peer_user().await;

    match payload {
        ResourceUpdateMsg::UpdatesResponse {
            resource_id,
            updates,
            ucan_token,
        } => {
            // Validate token
            let is_token_valid = validate_authority_for_update(
                resource_id,
                ucan_token,
                &peer_user.id,
                self.repo_ctx.clone(),
                &self.domain,
            ).await?;

            if !is_token_valid {
                return Err(ResourceSyncError::InvalidUpdateAuthority {
                    resource_id: resource_id.clone(),
                }.into());
            }

            // NEW: Extract author_id from UCAN token
            let author_id = extract_author_from_ucan(ucan_token)?;

            // Apply updates with author validation
            apply_updates(
                resource_id,
                updates,
                &user.id,
                &author_id,  // NEW: Pass author_id
                self.repo_ctx.clone(),
                &self.crypto_utils,
            ).await?;

            // ... rest of function ...
        }
        // ... other message types ...
    }
}

/// NEW HELPER: Extract author ID from UCAN token subject
fn extract_author_from_ucan(token: &str) -> Result<String, P2PError> {
    let ucan = crypto_utils::ucan_utils::validate_structure(token)
        .await
        .map_err(|e| P2PError::Custom(format!("Invalid UCAN: {}", e)))?;

    let subject = ucan.audience()
        .ok_or(P2PError::Custom("UCAN missing subject".to_string()))?;

    Ok(subject.to_string())
}
```

### 4.4 Form Submission Message Flow (NEW - Separate from Yjs Sync)

**Why Separate from Yjs Sync?**
- Form submissions are **one-way operations** (viewer → owner), not collaborative syncing
- They're **append-only** operations, not CRDT merges
- They target a **different resource** (submissions resource)
- Simpler validation logic (just check append permission, no authorship validation)

**NEW Message Types:**

```rust
// In network/src/p2p/messages.rs or similar

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FormMessage {
    /// Request to submit form data
    SubmitFormData {
        form_id: String,
        submission: FormSubmission,
        ucan_token: String,
    },

    /// Response after form submission
    SubmitFormDataResponse {
        success: bool,
        submission_id: Option<String>,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSubmission {
    pub id: String,
    pub viewer_id: String,
    pub timestamp: i64,
    pub form_data: serde_json::Value,
}
```

**Backend Handler:**

```rust
// In network/src/p2p/resource_sync.rs or form_handler.rs

pub async fn process_form_submission(
    &self,
    payload: &FormMessage,
) -> P2PResult<FormMessage> {
    match payload {
        FormMessage::SubmitFormData {
            form_id,
            submission,
            ucan_token,
        } => {
            // 1. Validate token has append permission for submissions resource
            let submissions_resource_id = format!("{}-submissions", form_id);

            let is_valid = validate_form_submission_token(
                form_id,
                &submissions_resource_id,
                ucan_token,
                self.repo_ctx.clone(),
            ).await?;

            if !is_valid {
                return Ok(FormMessage::SubmitFormDataResponse {
                    success: false,
                    submission_id: None,
                    error: Some("Invalid token or insufficient permissions".to_string()),
                });
            }

            // 2. Get submissions resource (owner's private resource)
            let user = self.get_local_user().await?;
            let (mut submissions_resource, encrypted_key) = get_resource(
                &submissions_resource_id,
                self.repo_ctx.clone(),
                &user.id,
                &self.crypto_utils,
            ).await?;

            // 3. Append submission to submissions doc
            let result = append_form_submission(
                &mut submissions_resource,
                submission,
            ).await;

            match result {
                Ok(()) => {
                    // 4. Save updated submissions resource
                    save_resource(
                        &submissions_resource_id,
                        &submissions_resource,
                        &encrypted_key,
                        self.repo_ctx.clone(),
                        &user.id,
                        &self.crypto_utils,
                    ).await?;

                    Ok(FormMessage::SubmitFormDataResponse {
                        success: true,
                        submission_id: Some(submission.id.clone()),
                        error: None,
                    })
                },
                Err(e) => {
                    Ok(FormMessage::SubmitFormDataResponse {
                        success: false,
                        submission_id: None,
                        error: Some(format!("Failed to save submission: {}", e)),
                    })
                }
            }
        },
        _ => Err(P2PError::Custom("Unexpected message type".to_string())),
    }
}

/// NEW FUNCTION: Validate token for form submission
async fn validate_form_submission_token(
    form_id: &str,
    submissions_resource_id: &str,
    token: &str,
    repo_ctx: Arc<RepositoryContext>,
) -> P2PResult<bool> {
    // Parse UCAN token
    let ucan = crypto_utils::ucan_utils::validate_structure(token).await?;

    // Check capabilities
    let capabilities = ucan.capabilities();

    // Must have append permission for submissions resource
    let has_permission = capabilities.iter().any(|cap| {
        let resource_uri = cap.resource();
        let action = cap.action();

        resource_uri.contains(submissions_resource_id) &&
        (action.contains("crud/append") || action.contains("crud/write"))
    });

    Ok(has_permission)
}

/// NEW FUNCTION: Append submission to submissions resource
async fn append_form_submission(
    submissions_resource: &mut DecryptedResource,
    submission: &FormSubmission,
) -> ServiceResult<()> {
    // Get form_submissions_doc
    let doc_data = submissions_resource.data
        .get("form_submissions_doc")
        .ok_or(ResourceServiceError::ParseError("Missing form_submissions_doc".to_string()))?;

    // Load Yjs doc
    let mut doc = Y::Doc::new();
    if let Some(updates) = doc_data.as_array() {
        if !updates.is_empty() {
            let update_bytes: Vec<u8> = updates.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            Y::applyUpdateV2(&mut doc, &update_bytes, "loading")?;
        }
    }

    // Get blocks map
    let blocks = doc.getMap("blocks");

    // Check if submission ID already exists (prevent duplicates)
    if blocks.contains(&submission.id) {
        return Err(ResourceServiceError::Conflict(
            format!("Submission {} already exists", submission.id)
        ));
    }

    // Create submission block
    let submission_block = serde_json::json!({
        "id": submission.id,
        "type": "form-submission",
        "viewer_id": submission.viewer_id,
        "timestamp": submission.timestamp,
        "form_data": submission.form_data,
    });

    // Add to blocks map
    blocks.set(&submission.id, submission_block);

    // Encode updated doc
    let updated_state = Y::encodeStateAsUpdateV2(&doc);

    // Update resource data
    submissions_resource.data.insert(
        "form_submissions_doc".to_string(),
        serde_json::to_value(updated_state.to_vec())?,
    );
    submissions_resource.data.insert(
        "last_modified".to_string(),
        serde_json::to_value(chrono::Utc::now().timestamp_millis())?,
    );

    Ok(())
}
```

**Frontend Integration:**

```typescript
// In Canvas.svelte or form handler

async function handleFormSubmit(formContainerId: string, formData: any) {
  console.log("📋 Form submitted:", formData);

  const resourceId = dataState.currentResourceId;
  if (!resourceId) {
    console.error("No resource selected");
    return;
  }

  const resource = dataState.getResourceById(resourceId);
  if (!resource || resource.resourceType !== 'form') {
    console.error("Not a form resource");
    return;
  }

  // Create submission
  const submissionId = `submission-${Date.now()}`;
  const submission = {
    id: submissionId,
    viewer_id: dataState.userDetails?.userId || 'anonymous',
    timestamp: Date.now(),
    form_data: formData
  };

  try {
    // Send via dedicated form submission message (NOT Yjs sync)
    const response = await window.ipc.invoke("submit_form_data", {
      form_id: resourceId,
      submission: submission,
      ucan_token: resource.ucanToken || await getFormUcanToken(resourceId)
    });

    if (response.success) {
      console.log("✅ Form submitted successfully:", response.submission_id);
      // Show success message to user
      showSuccessToast("Form submitted successfully!");
    } else {
      console.error("❌ Form submission failed:", response.error);
      showErrorToast(response.error || "Failed to submit form");
    }
  } catch (error) {
    console.error("❌ Form submission error:", error);
    showErrorToast("Failed to submit form");
  }
}
```

**Key Differences from Yjs Sync:**

| Aspect | Yjs Sync (Threads) | Form Submission |
|--------|-------------------|-----------------|
| Direction | Bidirectional (owner ↔ viewer) | Unidirectional (viewer → owner) |
| Merge Strategy | CRDT conflict resolution | Simple append |
| Message Type | `ResourceUpdateMsg::UpdatesResponse` | `FormMessage::SubmitFormData` |
| Target Resource | Same resource (thread_comments_doc) | Different resource (submissions) |
| Validation | Authorship (can only edit own) | Append-only (no edits) |
| Response | Ack with state vector | Success/failure with submission ID |

---

## 5. FRONTEND IMPLEMENTATION

### 5.1 YjsManager Changes (`yjsManager.ts`)

**Current:**
```typescript
export interface YjsDocuments {
  mainDoc: Y.Doc;
  blocks: Y.Map<any>;
  viewport: Y.Map<any>;
  awareness: Awareness;
}
```

**NEW - Support Multiple Docs:**
```typescript
export interface YjsDocuments {
  // Primary document
  mainDoc: Y.Doc;
  blocks: Y.Map<any>;
  viewport: Y.Map<any>;
  awareness: Awareness;

  // NEW: Secondary document (for threads)
  secondaryDoc?: Y.Doc;
  secondaryBlocks?: Y.Map<any>;
}

export class YjsManager {
  // ... existing code ...

  /**
   * NEW: Initialize with optional secondary doc for threads
   */
  initialize(resourceType?: string): YjsDocuments {
    this.destroy();

    const mainDoc = new Y.Doc({ gc: true, gcFilter: () => false });
    const blocks = mainDoc.getMap("blocks");
    const viewport = mainDoc.getMap("viewport");
    const awareness = new Awareness(mainDoc);

    // Initialize viewport
    if (viewport.size === 0) {
      viewport.set("x", 0);
      viewport.set("y", 0);
      viewport.set("zoom", 1);
    }

    // Set up update listeners...

    let secondaryDoc: Y.Doc | undefined;
    let secondaryBlocks: Y.Map<any> | undefined;

    // NEW: Create secondary doc for threads
    if (resourceType === 'noticeboard') {
      secondaryDoc = new Y.Doc({ gc: true, gcFilter: () => false });
      secondaryBlocks = secondaryDoc.getMap("blocks");

      // Set up update listener for secondary doc
      if (this.config.onUpdate) {
        secondaryDoc.on("updateV2", (update: Uint8Array, origin: any) => {
          if (origin !== "sync" && origin !== "loading") {
            // Emit with doc_type identifier
            this.config.onUpdate!(update, origin, "thread_comments_doc");
          }
        });
      }
    }

    this.documents = {
      mainDoc,
      blocks,
      viewport,
      awareness,
      secondaryDoc,
      secondaryBlocks,
    };

    return this.documents;
  }

  /**
   * NEW: Apply update to specific doc
   */
  applyUpdate(update: Uint8Array | number[], origin: any = 'sync', docType?: string): void {
    if (!this.documents) return;
    const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);

    if (docType === 'thread_comments_doc' && this.documents.secondaryDoc) {
      Y.applyUpdateV2(this.documents.secondaryDoc, updateArray, origin);
    } else {
      Y.applyUpdateV2(this.documents.mainDoc, updateArray, origin);
    }
  }

  /**
   * NEW: Get state as update for both docs
   */
  getStateAsUpdateMulti(): { mainDoc: Uint8Array, secondaryDoc?: Uint8Array } {
    if (!this.documents) {
      return { mainDoc: new Uint8Array() };
    }

    const result = {
      mainDoc: Y.encodeStateAsUpdateV2(this.documents.mainDoc),
      secondaryDoc: this.documents.secondaryDoc
        ? Y.encodeStateAsUpdateV2(this.documents.secondaryDoc)
        : undefined
    };

    return result;
  }
}
```

### 5.2 BlocksuiteCoordinator Changes (`blocksuiteCoordinator.ts`)

**NEW - Load/Save Split Docs:**
```typescript
export class BlocksuiteCoordinator {
  private currentDocKey: string = 'main_doc';
  private resourceType: string = 'website';

  /**
   * Load blocksuite data - handles split docs for threads
   */
  loadBlocksuite(data: any, resourceType?: string): void {
    if (!data) {
      console.warn("⚠️ No data provided to loadBlocksuite");
      return;
    }

    this.resourceType = resourceType || this.detectResourceType(data);

    try {
      console.log("🔄 Loading blocksuite data for type:", this.resourceType);

      // Reinitialize with resource type
      const docs = this.yjsManager.initialize(this.resourceType);
      this.yjsManager.setUserInfo(this.config.userInfo);

      // Set up ready listener
      docs.mainDoc.once('afterAllTransactions', () => {
        console.log("✅ Document transactions complete");
        document.dispatchEvent(new CustomEvent('blocksuite-ready', {
          detail: { resourceId: this.config.userInfo.id }
        }));
      });

      // Detect and apply document updates
      if (this.resourceType === 'noticeboard') {
        // Load thread_doc (main post)
        if (data.thread_doc) {
          const threadUpdates = Array.isArray(data.thread_doc)
            ? new Uint8Array(data.thread_doc)
            : (data.thread_doc.updates ? new Uint8Array(data.thread_doc.updates) : null);

          if (threadUpdates && threadUpdates.length > 0) {
            console.log(`📥 Applying thread_doc updates (${threadUpdates.length} bytes)`);
            this.yjsManager.applyUpdate(threadUpdates, "loading", "thread_doc");
          }
        }

        // Load thread_comments_doc (comments)
        if (data.thread_comments_doc) {
          const commentsUpdates = Array.isArray(data.thread_comments_doc)
            ? new Uint8Array(data.thread_comments_doc)
            : (data.thread_comments_doc.updates ? new Uint8Array(data.thread_comments_doc.updates) : null);

          if (commentsUpdates && commentsUpdates.length > 0) {
            console.log(`📥 Applying thread_comments_doc updates (${commentsUpdates.length} bytes)`);
            this.yjsManager.applyUpdate(commentsUpdates, "loading", "thread_comments_doc");
          }
        }

        this.currentDocKey = 'thread_doc';
      } else {
        // Standard single-doc loading (website, form)
        let docKey: string | null = null;
        let updates: Uint8Array | null = null;

        if (data.blocksuite_doc) {
          docKey = 'blocksuite_doc';
          updates = Array.isArray(data.blocksuite_doc)
            ? new Uint8Array(data.blocksuite_doc)
            : (data.blocksuite_doc.updates ? new Uint8Array(data.blocksuite_doc.updates) : null);
        } else if (data.form_doc) {
          docKey = 'form_doc';
          updates = Array.isArray(data.form_doc)
            ? new Uint8Array(data.form_doc)
            : (data.form_doc.updates ? new Uint8Array(data.form_doc.updates) : null);
        }

        if (docKey && updates && updates.length > 0) {
          console.log(`📥 Applying ${docKey} updates (${updates.length} bytes)`);
          this.yjsManager.applyUpdate(updates, "loading");
        }

        if (docKey) {
          this.currentDocKey = docKey;
        }
      }
    } catch (error) {
      console.error("❌ Error loading blocksuite:", error);
    }
  }

  /**
   * Save blocksuite data - handles split docs for threads
   */
  saveBlocksuite(): any {
    console.log("💾 saveBlocksuite called, resourceType:", this.resourceType);

    const docs = this.yjsManager.getDocuments();
    if (!docs) {
      console.error("❌ No documents available");
      return null;
    }

    if (this.resourceType === 'noticeboard' && docs.secondaryDoc) {
      // Save both thread_doc and thread_comments_doc
      const mainUpdates = Y.encodeStateAsUpdateV2(docs.mainDoc);
      const secondaryUpdates = Y.encodeStateAsUpdateV2(docs.secondaryDoc);

      console.log("📤 Saving split docs:", {
        thread_doc: mainUpdates.length,
        thread_comments_doc: secondaryUpdates.length
      });

      return {
        thread_doc: Array.from(mainUpdates),
        thread_comments_doc: Array.from(secondaryUpdates),
        last_modified: Date.now()
      };
    } else {
      // Standard single-doc save
      const mainUpdates = Y.encodeStateAsUpdateV2(docs.mainDoc);

      return {
        [this.currentDocKey]: Array.from(mainUpdates),
        last_modified: Date.now()
      };
    }
  }

  private detectResourceType(data: any): string {
    if (data.thread_doc || data.thread_comments_doc) return 'noticeboard';
    if (data.form_doc) return 'form';
    if (data.blocksuite_doc) return 'website';
    return 'website';
  }
}
```

### 5.3 NoticeBoardBuilder Changes (`NoticeBoardBuilder.svelte`)

**Modify to use split blocks:**

```typescript
<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";
  import { dataState } from "../state";
  import NavigationPanel from "../components/NavigationPanel.svelte";
  import ThreadPost from "./ThreadPost.svelte";
  import CommentList from "./CommentList.svelte";
  import type { YjsDocuments } from "./yjsManager";

  let yDocs: YjsDocuments | null = null;

  // NEW: Separate blocks for post and comments
  let threadPostBlocks = $state<Map<string, any>>(new Map());
  let commentBlocks = $state<Map<string, any>>(new Map());

  let autoSaveInterval: number | null = null;

  const hasResource = $derived(!!dataState.currentResourceId);

  // Get the thread post block from mainDoc blocks
  const threadPost = $derived(() => {
    for (const [id, block] of threadPostBlocks) {
      if (block.type === 'thread-post') {
        return block;
      }
    }
    return null;
  });

  // Get comment blocks from secondaryDoc blocks
  const comments = $derived(() => {
    const commentList: any[] = [];
    for (const [id, block] of commentBlocks) {
      if (block.type === 'comment') {
        commentList.push(block);
      }
    }
    return commentList.sort((a, b) => (a.order || 0) - (b.order || 0));
  });

  onMount(async () => {
    console.log("🚀 Initializing Thread Builder...");

    // Auto-save every 10 seconds
    autoSaveInterval = window.setInterval(async () => {
      const currentResourceId = dataState.currentResourceId;
      if (currentResourceId) {
        try {
          console.log("💾 Auto-saving thread:", currentResourceId);
          await dataState.saveCurrentResource(currentResourceId);
          console.log("✅ Auto-save completed");
        } catch (error) {
          console.error("❌ Auto-save failed:", error);
        }
      }
    }, 10000);

    console.log("✅ Thread Builder initialized with auto-save!");
  });

  $effect(() => {
    const resourceId = dataState.currentResourceId;
    console.log("🔄 Resource changed:", resourceId);

    if (!resourceId) {
      yDocs = null;
      threadPostBlocks = new Map();
      commentBlocks = new Map();
      return;
    }

    untrack(() => {
      const coordinator = dataState.getBlocksuiteCoordinator();
      if (!coordinator) {
        console.error("❌ No coordinator available");
        return;
      }

      const docs = coordinator.getDocuments();
      if (!docs) {
        console.error("❌ No Yjs documents available");
        return;
      }

      yDocs = docs;

      console.log("📦 Yjs documents received:", {
        hasMainDoc: !!docs.mainDoc,
        hasSecondaryDoc: !!docs.secondaryDoc
      });

      // NEW: Subscribe to main doc blocks (thread post)
      const mainBlocksObserver = () => {
        if (!yDocs) return;
        const newBlocks = new Map();
        yDocs.blocks.forEach((value, key) => {
          newBlocks.set(key, value);
        });
        threadPostBlocks = newBlocks;
        console.log("📦 Thread post blocks updated:", threadPostBlocks.size);
      };

      // NEW: Subscribe to secondary doc blocks (comments)
      const secondaryBlocksObserver = () => {
        if (!yDocs || !yDocs.secondaryBlocks) return;
        const newBlocks = new Map();
        yDocs.secondaryBlocks.forEach((value, key) => {
          newBlocks.set(key, value);
        });
        commentBlocks = newBlocks;
        console.log("📦 Comment blocks updated:", commentBlocks.size);
      };

      docs.blocks.observe(mainBlocksObserver);
      if (docs.secondaryBlocks) {
        docs.secondaryBlocks.observe(secondaryBlocksObserver);
      }

      // Initial load
      mainBlocksObserver();
      secondaryBlocksObserver();

      // Create initial thread post if needed
      if (!Array.from(threadPostBlocks.values()).some(b => b.type === 'thread-post')) {
        console.log("📝 Creating initial thread post block");
        createThreadPost();
      }
    });

    return () => {
      // Cleanup
    };
  });

  onDestroy(() => {
    if (autoSaveInterval !== null) {
      clearInterval(autoSaveInterval);
      autoSaveInterval = null;
    }
  });

  function createThreadPost() {
    if (!yDocs) return;
    const id = `thread-post-${Date.now()}`;
    const newBlock = {
      id,
      type: 'thread-post',
      content: '',
      mode: 'markdown',
      css: '',
      author: 'Owner',
      timestamp: new Date().toISOString(),
      order: 0
    };
    // Add to main doc
    yDocs.blocks.set(id, newBlock);
  }

  function updateThreadPost(updates: any) {
    if (!yDocs || !threadPost()) return;
    const post = threadPost();
    if (post) {
      // Update in main doc
      yDocs.blocks.set(post.id, { ...post, ...updates });
    }
  }

  function addComment(content: string, mode: string, css: string, parentId?: string) {
    if (!yDocs || !yDocs.secondaryBlocks) return;
    const id = `comment-${Date.now()}`;
    const newBlock = {
      id,
      type: 'comment',
      content,
      mode,
      css: css || '',
      author: 'Owner',  // Will be set by backend from UCAN token
      timestamp: new Date().toISOString(),
      parentId: parentId || null,
      order: commentBlocks.size
    };
    // NEW: Add to secondary doc (comments)
    yDocs.secondaryBlocks.set(id, newBlock);
  }

  function updateComment(commentId: string, updates: any) {
    if (!yDocs || !yDocs.secondaryBlocks) return;
    const comment = commentBlocks.get(commentId);
    if (comment) {
      // Update in secondary doc
      yDocs.secondaryBlocks.set(commentId, { ...comment, ...updates });
    }
  }

  function deleteComment(commentId: string) {
    if (!yDocs || !yDocs.secondaryBlocks) return;
    // Delete from secondary doc
    yDocs.secondaryBlocks.delete(commentId);
  }
</script>

{#if !hasResource}
  <div class="empty-state">
    <NavigationPanel />
    <div class="empty-message">
      <h2>No thread selected</h2>
      <p>Select a thread from the sidebar or create a new one to get started.</p>
    </div>
  </div>
{:else}
  <div class="builder-container">
    <NavigationPanel />
    <div class="main-content">
      <div class="thread-container">
        {#if threadPost()}
          <ThreadPost
            post={threadPost()}
            onUpdate={updateThreadPost}
          />
        {/if}

        <CommentList
          comments={comments()}
          onAddComment={addComment}
          onUpdateComment={updateComment}
          onDeleteComment={deleteComment}
        />
      </div>
    </div>
  </div>
{/if}

<!-- ... styles ... -->
```

### 5.4 Form Submission Handler (`Canvas.svelte`)

**NEW: Submit to node instead of console.log:**

```typescript
async function handleFormSubmit(formContainerId: string, formData: any) {
  console.log("📋 Form submitted:", formData);

  // Get current resource
  const resourceId = dataState.currentResourceId;
  if (!resourceId) {
    console.error("No resource selected");
    return;
  }

  // Get resource details
  const resource = dataState.getResourceById(resourceId);
  if (!resource || resource.resourceType !== 'form') {
    console.error("Not a form resource");
    return;
  }

  // Create submission block
  const submissionId = `submission-${Date.now()}`;
  const submission = {
    id: submissionId,
    type: 'form-submission',
    viewer_id: dataState.userDetails?.userId || 'anonymous',
    timestamp: Date.now(),
    form_data: formData
  };

  // TODO: Get UCAN token for this form
  const ucanToken = await getFormUcanToken(resourceId);

  // Send to node
  try {
    await sendMessage("submitFormData", {
      submissionResourceId: `${resourceId}-submissions`,
      submission,
      ucanToken
    });

    console.log("✅ Form submitted successfully");
    // Show success message to user
  } catch (error) {
    console.error("❌ Form submission failed:", error);
    // Show error message to user
  }
}
```

---

## 6. IMPLEMENTATION PRIORITIES

### Phase 1: Backend Foundation (Week 1)
1. **Split thread documents in backend**
   - Modify `blocksuiteUtils.ts` to create both `thread_doc` and `thread_comments_doc`
   - Update `createNoticeBoardDoc()` to initialize both docs
   - Test loading/saving split docs

2. **Resource-specific token generation**
   - Implement `prepare_resource_for_viewer()` with resource type switching
   - Add capability generation for each resource type
   - Test token validation

3. **Authorization validation**
   - Implement `validate_thread_comment_authorship()`
   - Implement `validate_form_submission_append_only()`
   - Add `extract_author_from_ucan()` helper
   - Integrate into `apply_updates()` flow

### Phase 2: Frontend Split Docs (Week 1-2)
1. **YjsManager multi-doc support**
   - Add `secondaryDoc` and `secondaryBlocks` to interface
   - Implement secondary doc initialization for threads
   - Add `applyUpdate()` doc-type routing
   - Test with manual Yjs docs

2. **BlocksuiteCoordinator split doc handling**
   - Modify `loadBlocksuite()` to handle thread_doc + thread_comments_doc
   - Modify `saveBlocksuite()` to save both docs
   - Add resource type detection
   - Test save/load cycle

3. **NoticeBoardBuilder split blocks**
   - Separate `threadPostBlocks` and `commentBlocks` state
   - Subscribe to both docs
   - Route updates to correct doc (post → mainDoc, comment → secondaryDoc)
   - Test adding/editing comments

### Phase 3: Forms Submissions (Week 2)
1. **Backend submissions resource**
   - Implement `create_or_get_submissions_resource()`
   - Create private folder for submissions
   - Generate token with append permission for submissions resource
   - Test resource creation

2. **NEW: Form submission message handler**
   - Define `FormMessage` enum with `SubmitFormData` and `SubmitFormDataResponse`
   - Implement `process_form_submission()` handler
   - Implement `validate_form_submission_token()`
   - Implement `append_form_submission()`
   - Test submission validation and storage

3. **Frontend form submission**
   - Modify `Canvas.svelte` to use dedicated form submission IPC
   - Get UCAN token from resource
   - Create submission object
   - Send via `submit_form_data` IPC (NOT Yjs sync)
   - Handle success/error responses

4. **Owner submissions view**
   - Create `FormSubmissionsViewer.svelte` component
   - List submissions from private resource
   - Display submission data in table/cards
   - Test viewing submissions

### Phase 4: Viewer Mode UI (Week 2-3)
1. **NoticeBoardViewer component**
   - Create read-only thread post display
   - Add comment input (if viewer has permission)
   - Display nested comments
   - Test commenting as viewer

2. **Connection string generation**
   - Generate per-resource connection strings
   - Include UCAN token and resource metadata
   - Test connecting as viewer

3. **Sync flow integration**
   - Pull from node on viewer load
   - Push comments to node
   - Test CRDT merging

### Phase 5: Testing & Polish (Week 3)
1. **Authorization testing**
   - Test viewer cannot edit thread post
   - Test viewer can only edit own comments
   - Test form submission append-only

2. **Concurrent updates testing**
   - Multiple viewers commenting simultaneously
   - CRDT conflict resolution
   - Eventual consistency

3. **UI polish**
   - Loading states
   - Error messages
   - Success feedback
   - Connection status indicators

---

## 7. KEY FILES TO MODIFY

### Backend (Rust)
```
services/src/
├─ node_service.rs
│  └─ prepare_resource_for_viewer() - Add resource-specific tokens
│  └─ create_or_get_submissions_resource() - NEW function
│
├─ resource_service.rs
│  └─ apply_updates() - Add author_id parameter
│  └─ validate_thread_comment_authorship() - NEW function
│  └─ validate_form_submission_append_only() - NEW function (for Yjs sync path)
│  └─ parse_yjs_update_blocks() - NEW helper
│
network/src/p2p/
├─ messages.rs (or new file)
│  └─ FormMessage enum - NEW message types
│  └─ FormSubmission struct - NEW struct
│
├─ resource_sync.rs
│  └─ process_resource_update_message() - Extract author from token (for threads)
│  └─ process_form_submission() - NEW handler for form submissions
│  └─ validate_form_submission_token() - NEW validator
│  └─ append_form_submission() - NEW append function
│  └─ extract_author_from_ucan() - NEW helper
│
core/src/models/
└─ document.rs
   └─ YjsDocExt trait - Add authorship validation methods
```

### Frontend (TypeScript/Svelte)
```
sthalam/frontend/desktop/src/
├─ lib/
│  ├─ yjsManager.ts
│  │  └─ YjsDocuments interface - Add secondaryDoc/secondaryBlocks
│  │  └─ initialize() - Add resource type parameter
│  │  └─ applyUpdate() - Add doc type routing
│  │  └─ getStateAsUpdateMulti() - NEW method
│  │
│  ├─ blocksuiteCoordinator.ts
│  │  └─ loadBlocksuite() - Handle split docs
│  │  └─ saveBlocksuite() - Save split docs
│  │  └─ detectResourceType() - NEW helper
│  │
│  ├─ NoticeBoardBuilder.svelte
│  │  └─ Split threadPostBlocks and commentBlocks
│  │  └─ Subscribe to both docs
│  │  └─ Route updates to correct doc
│  │
│  ├─ Canvas.svelte
│  │  └─ handleFormSubmit() - NEW: Send via dedicated IPC (not Yjs sync)
│  │  └─ showSuccessToast() / showErrorToast() - NEW: User feedback
│  │
│  ├─ FormSubmissionsViewer.svelte - NEW component
│  │  └─ Load submissions resource
│  │  └─ Display submissions in table/cards
│  │
│  └─ NoticeBoardViewer.svelte - NEW component
│
├─ utils/
│  └─ blocksuiteUtils.ts
│     └─ createNoticeBoardDoc() - Initialize both docs
│
└─ state/
   └─ data.svelte.ts
      └─ submitFormData() - NEW helper for form submissions
      └─ loadFormSubmissions() - NEW helper for owner
```

---

## 8. TESTING CHECKLIST

### Thread Split Docs
- [ ] Create thread resource → generates thread_doc + thread_comments_doc
- [ ] Save thread → both docs saved to backend
- [ ] Load thread → both docs loaded from backend
- [ ] Add comment → goes to thread_comments_doc
- [ ] Edit thread post → goes to thread_doc
- [ ] Edit own comment → allowed
- [ ] Edit other's comment → rejected

### Forms Submissions
- [ ] Create form → generates form_doc + submissions resource
- [ ] Submit form as viewer → submission added to private resource
- [ ] Submit form again → new submission added (not overwriting)
- [ ] Try to modify submission → rejected (append-only)
- [ ] View submissions as owner → see all submissions
- [ ] Try to view submissions as viewer → access denied

### Authorization
- [ ] Viewer with thread token → can read post, can write comments
- [ ] Viewer with form token → can read form, can submit (not view submissions)
- [ ] Viewer with website token → can only read
- [ ] Token without capability → rejected
- [ ] Expired token → rejected
- [ ] Invalid token signature → rejected

### CRDT & Sync
- [ ] Two viewers comment simultaneously → both appear (no conflicts)
- [ ] Owner and viewer edit different parts → merges correctly
- [ ] Offline comment → syncs when back online
- [ ] State vector sync → only missing updates transferred

---

## 9. ARCHITECTURE DIAGRAM

```
┌─────────────────────────────────────────────────────────────────┐
│                         OWNER (Creator)                          │
├─────────────────────────────────────────────────────────────────┤
│  Builder Mode:                                                   │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐│
│  │ WebsiteBuilder  │  │NoticeBoardBuilder│  │WebsiteBuilder   ││
│  │ (Website/Form)  │  │   (Thread)       │  │   (Form)        ││
│  └────────┬────────┘  └────────┬─────────┘  └────────┬────────┘│
│           │                    │                     │          │
│           ▼                    ▼                     ▼          │
│  ┌────────────────────────────────────────────────────────────┐│
│  │           BlocksuiteCoordinator (Yjs Manager)              ││
│  │  ┌──────────────┐  ┌─────────────────────────────────────┐││
│  │  │ blocksuite_doc│  │thread_doc + thread_comments_doc    │││
│  │  └──────────────┘  └─────────────────────────────────────┘││
│  │  ┌──────────────┐  ┌─────────────────────────────────────┐││
│  │  │   form_doc    │  │form_submissions_doc (private)      │││
│  │  └──────────────┘  └─────────────────────────────────────┘││
│  └────────────────────────────────────────────────────────────┘│
│           │                                                      │
│           ▼ Publish with UCAN tokens                            │
│  ┌────────────────────────────────────────────────────────────┐│
│  │              SOVEREIGN NODE (Owner's Node)                  ││
│  │  - Stores published resources                               ││
│  │  - Validates UCAN tokens                                    ││
│  │  - Mediates viewer updates                                  ││
│  │  - Authoritative source for comments/submissions            ││
│  └────────┬───────────────────────────────────────────────────┘│
└───────────┼─────────────────────────────────────────────────────┘
            │
            │ UCAN Token (resource-specific capabilities)
            │
┌───────────▼─────────────────────────────────────────────────────┐
│                      VIEWERS (Public)                            │
├─────────────────────────────────────────────────────────────────┤
│  Viewer Mode:                                                    │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐│
│  │   Website       │  │    Thread       │  │      Form       ││
│  │  (READ-ONLY)    │  │ (READ+COMMENT)  │  │  (READ+SUBMIT)  ││
│  └─────────────────┘  └────────┬────────┘  └────────┬────────┘│
│                                 │                     │          │
│                      Add Comment│          Submit Form│          │
│                                 ▼                     ▼          │
│                    ┌────────────────────────────────────┐       │
│                    │    Send to Sovereign Node          │       │
│                    │  - Include UCAN token              │       │
│                    │  - Validate capabilities           │       │
│                    │  - Validate authorship (threads)   │       │
│                    │  - Validate append-only (forms)    │       │
│                    └────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────────┘

Token Flow:
  Owner → Generate token with capabilities → Embed in connection string
         → Viewer connects with token → Sovereign validates → Access granted

Comment Flow (Threads):
  Viewer → Add comment to thread_comments_doc → Send to node with token
         → Node validates author → Node merges to thread_comments_doc
         → Owner syncs from node → Sees new comment

Submission Flow (Forms):
  Viewer → Fill form → Create FormSubmission object → Send FormMessage::SubmitFormData to node
         → Node validates token (has append permission for submissions resource)
         → Node appends to private submissions resource (append_form_submission)
         → Node responds with FormMessage::SubmitFormDataResponse (success/error)
         → Owner can load submissions resource → Views in dashboard
```

---

## 10. NEXT SESSION STARTING POINT

When starting implementation in the next chat:

1. **Start with backend token generation:**
   - File: `/home/abe/osvauld/services/src/node_service.rs`
   - Function: `prepare_resource_for_viewer()`
   - Task: Add match statement for resource types with different capabilities

2. **Then move to split docs:**
   - File: `/home/abe/osvauld/sthalam/frontend/desktop/src/utils/blocksuiteUtils.ts`
   - Function: `createNoticeBoardDoc()`
   - Task: Initialize both `thread_doc` and `thread_comments_doc`

3. **Reference existing patterns:**
   - Look at how `form_doc` and `blocksuite_doc` are handled in `blocksuiteCoordinator.ts:loadBlocksuite()`
   - Follow the same pattern for `thread_doc` + `thread_comments_doc`

4. **Test incrementally:**
   - After each change, test save/load cycle
   - Verify both docs are present in database
   - Check frontend correctly separates blocks

**Command to start testing:**
```bash
cd /home/abe/osvauld/sthalam
cargo tauri dev
```

---

This implementation plan provides a complete architecture for viewer interactions with proper authorization, split documents for threads, and separate submissions storage for forms. All using CRDT-based merging for conflict-free collaboration through the sovereign node.
