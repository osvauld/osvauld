// Permissions Configuration
// Defines capability templates for folders and resources

// ========== FOLDER TEMPLATES ==========

export const FOLDER_TEMPLATE = {
  owner_template: {
    capabilities: {
      "own": "own",
      "get_share_link": "get_share_link",
      "add_resources": "add_resources",
      "crud/read": "crud/read",
      "crud/update": "crud/update",
      "crud/delete": "crud/delete",
      "share_folder": "share_folder",
    },
    delegation: {
      node: {
        capabilities: {
          "get_share_link": "get_share_link",
          "add_resources": "add_resources",
          "crud/read": "crud/read",
          "share_folder": "share_folder",
        },
      },
      viewer: {
        capabilities: {
          "crud/read": "crud/read",
          "request_resources": "request_resources",
          "get_share_link": "get_share_link",
        },
      },
    },
  },
};

// ========== RESOURCE TEMPLATES ==========

export const RESOURCE_TEMPLATE = {
  owner_template: {
    capabilities: {
      "template_doc": "collaborator",
      "content_doc": "collaborator",
      "user_content_doc": "collaborator",
      "collaborative_doc": "collaborator",
      "submissions_doc": "collaborator",
      "static_assets": "collaborator",
    },
    doc_types: {
      "static_assets": "asset",
      "template_doc": "crdt",
      "content_doc": "crdt",
      "user_content_doc": "crdt",
      "collaborative_doc": "crdt",
      "submissions_doc": "crdt",
    },
    sync: {
      local_only: ["user_content_doc"],
    },
    delegation: {
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
