// Permissions Configuration - UCAN v3 Facts-Only Architecture
// Defines capability templates for folders and resources
//
// ⚠️  IMPORTANT: This is the source of truth for permissions in UCAN tokens.
// All authorization is stored in token facts - no URI capabilities are used.
//
// Facts-Only Architecture:
// - All permissions stored in facts.operations, facts.documents, facts.auth_capabilities
// - CEL rules in facts.cel_rules provide dynamic authorization
// - Relationships (owner/node/viewer) stored in facts.relationship
//
// Backend transforms these templates into UCAN token facts.

// ========== FOLDER TEMPLATES ==========
//
// Folder operations (no document-based capabilities, only operations on the folder)

export const FOLDER_TEMPLATE = {
  owner_template: {
    // Operations available on this folder (owner has all of them)
    operations: {
      "own": "allow",
      "get_share_link": "allow",
      "add_resources": "allow",
      "share_folder": "allow",
    },
    delegation: {
      // What operations can be delegated to a node
      node: {
        token_type: "folder_share",  // Data-driven: backend uses this for delegated token
        operations: {
          "get_share_link": "allow",
          "add_resources": "allow",
          "share_folder": "allow",
        },
        // CEL-based authorization capabilities
        auth_capabilities: {
          can_connect: true,
          persist_share: true,
          can_delegate: false,
          sync_enabled: true,
        },
        relationship: "node",
        cel_rules: {
          persist_share: "auth_capabilities.persist_share == true && relationship == 'node'",
          can_connect: "auth_capabilities.can_connect == true",
          can_delegate: "auth_capabilities.can_delegate == true && operations.own == 'allow'",
          sync_enabled: "auth_capabilities.sync_enabled == true && operations.get_share_link == 'allow'",
        },
      },
      // What operations can be delegated to a viewer
      viewer: {
        token_type: "folder_viewer",  // Data-driven: backend uses this for delegated token
        operations: {
          "request_resources": "allow",
          "get_share_link": "allow",
        },
        // CEL-based authorization capabilities
        auth_capabilities: {
          can_connect: true,
          persist_share: false,
          can_delegate: false,
          sync_enabled: false,
        },
        relationship: "viewer",
        cel_rules: {
          persist_share: "auth_capabilities.persist_share == true && relationship == 'node'",
          can_connect: "auth_capabilities.can_connect == true",
          can_delegate: "auth_capabilities.can_delegate == true && operations.own == 'allow'",
          sync_enabled: "auth_capabilities.sync_enabled == true && operations.get_share_link == 'allow'",
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
    // Operations on the resource itself
    operations: {
      "own": "allow",
      "share_resource": "allow",  // Permission to share this resource with others
    },
    // Documents in this resource (map format: doc_name -> {capability, type})
    documents: {
      "template_doc": {
        capability: "collaborator",
        type: "crdt",
      },
      "content_doc": {
        capability: "collaborator",
        type: "crdt",
      },
      "user_content_doc": {
        capability: "collaborator",
        type: "crdt",
      },
      "collaborative_doc": {
        capability: "collaborator",
        type: "crdt",
      },
      "submissions_doc": {
        capability: "collaborator",
        type: "crdt",
      },
      "static_assets": {
        capability: "collaborator",
        type: "asset",
      },
    },
    // Sync behavior for owner
    sync: {
      local_only: ["user_content_doc"],
    },
    delegation: {
      // What the node receives
      node: {
        token_type: "resource_share",  // Data-driven: backend uses this for delegated token
        operations: {
          "share_resource": "allow",  // Node can also share resources
        },
        documents: {
          "template_doc": {
            capability: "collaborator",
            type: "crdt",
          },
          "content_doc": {
            capability: "collaborator",
            type: "crdt",
          },
          "user_content_doc": {
            capability: "collaborator",
            type: "crdt",
          },
          "collaborative_doc": {
            capability: "collaborator",
            type: "crdt",
          },
          "submissions_doc": {
            capability: "collaborator",
            type: "crdt",
          },
          "static_assets": {
            capability: "collaborator",
            type: "asset",
          },
        },
        sync: {
          local_only: ["user_content_doc"],
        },
        // CEL-based authorization capabilities
        auth_capabilities: {
          can_connect: true,
          persist_share: true,
          can_delegate: false,
          sync_enabled: true,
        },
        relationship: "node",
        cel_rules: {
          persist_share: "auth_capabilities.persist_share == true && relationship == 'node'",
          can_connect: "auth_capabilities.can_connect == true",
          can_delegate: "auth_capabilities.can_delegate == true && operations.own == 'allow'",
          sync_enabled: "auth_capabilities.sync_enabled == true",
        },
      },
      // What viewers receive
      viewer: {
        token_type: "resource_viewer",  // Data-driven: backend uses this for delegated token
        operations: {},  // Viewers have no resource-level operations
        documents: {
          "template_doc": {
            capability: "viewer",
            type: "crdt",
          },
          "content_doc": {
            capability: "viewer",
            type: "crdt",
          },
          "collaborative_doc": {
            capability: "collaborator",
            type: "crdt",
          },
          "submissions_doc": {
            capability: "submitter",
            type: "crdt",
          },
          "static_assets": {
            capability: "viewer",
            type: "asset",
          },
        },
        sync: {
          local_only: ["user_content_doc"],
          no_incoming_updates: ["submissions_doc"],
          send_full_snapshot: ["submissions_doc"],
        },
        // CEL-based authorization capabilities
        auth_capabilities: {
          can_connect: true,
          persist_share: false,
          can_delegate: false,
          sync_enabled: false,
        },
        relationship: "viewer",
        cel_rules: {
          persist_share: "auth_capabilities.persist_share == true && relationship == 'node'",
          can_connect: "auth_capabilities.can_connect == true",
          can_delegate: "auth_capabilities.can_delegate == true && operations.own == 'allow'",
          sync_enabled: "auth_capabilities.sync_enabled == true",
        },
      },
    },
  },
};

// ========== TYPE EXPORTS ==========

export type FolderTemplate = typeof FOLDER_TEMPLATE;
export type ResourceTemplate = typeof RESOURCE_TEMPLATE;
