// Permissions Configuration
// Defines capability templates for folders and resources
//
// ⚠️  IMPORTANT: This is the source of truth for WHAT permissions exist.
// HOW they are encoded (URIs) is handled by the backend using the uri module.
//
// URI Standard (backend responsibility):
// - Folder operations: `domain:folder:id:operation`
// - Resource capabilities: `domain:resource:id:doc_name`
//
// Frontend only specifies WHAT operations/documents exist and their access levels.

// ========== FOLDER TEMPLATES ==========
//
// Folder operations (no document-based capabilities, only operations on the folder)

export const FOLDER_TEMPLATE = {
  owner_template: {
    // Operations available on this folder (owner has all of them)
    capabilities: {
      "own": "allow",
      "get_share_link": "allow",
      "add_resources": "allow",
      "share_folder": "allow",
    },
    delegation: {
      // What operations can be delegated to a node
      node: {
        capabilities: {
          "get_share_link": "allow",
          "add_resources": "allow",
          "share_folder": "allow",
        },
      },
      // What operations can be delegated to a viewer
      viewer: {
        capabilities: {
          "request_resources": "allow",
          "get_share_link": "allow",
        },
      },
    },
  },
};

// ========== RESOURCE TEMPLATES ==========
//
// Resource documents and their capabilities across different roles

export const RESOURCE_TEMPLATE = {
  owner_template: {
    // Documents in this resource and their access levels for owner
    capabilities: {
      "template_doc": "collaborator",
      "content_doc": "collaborator",
      "user_content_doc": "collaborator",
      "collaborative_doc": "collaborator",
      "submissions_doc": "collaborator",
      "static_assets": "collaborator",
    },
    // Document types (CRDT or Asset)
    doc_types: {
      "static_assets": "asset",
      "template_doc": "crdt",
      "content_doc": "crdt",
      "user_content_doc": "crdt",
      "collaborative_doc": "crdt",
      "submissions_doc": "crdt",
    },
    // Sync behavior for owner
    sync: {
      local_only: ["user_content_doc"],
    },
    delegation: {
      // What the node receives
      node: {
        capabilities: {
          "template_doc": "collaborator",
          "content_doc": "collaborator",
          "user_content_doc": "collaborator",
          "collaborative_doc": "collaborator",
          "submissions_doc": "collaborator",
          "static_assets": "collaborator",
        },
        sync: {
          local_only: ["user_content_doc"],
        },
      },
      // What viewers receive
      viewer: {
        capabilities: {
          "template_doc": "viewer",
          "content_doc": "viewer",
          "collaborative_doc": "collaborator",
          "submissions_doc": "submitter",
          "static_assets": "viewer",
        },
        sync: {
          local_only: ["user_content_doc"],
          no_incoming_updates: ["submissions_doc"],
          send_full_snapshot: ["submissions_doc"],
        },
      },
    },
  },
};

// ========== TYPE EXPORTS ==========

export type FolderTemplate = typeof FOLDER_TEMPLATE;
export type ResourceTemplate = typeof RESOURCE_TEMPLATE;
