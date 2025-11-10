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
  },
  node_template: {
    capabilities: {
      "get_share_link": "get_share_link",
      "add_resources": "add_resources",
      "crud/read": "crud/read",
      "share_folder": "share_folder",
    },
  },
  viewer_template: {
    capabilities: {
      "crud/read": "crud/read",
      "request_resources": "request_resources",
      "get_share_link": "get_share_link",
    },
  },
};

// ========== RESOURCE TEMPLATES ==========

export const RESOURCE_TEMPLATE = {
  owner_template: {
    capabilities: {
      "template_doc": "crud/merge",
      "content_doc": "crud/merge",
      "user_content_doc": "crud/merge",
      "collaborative_doc": "crud/merge",
      "submissions_doc": "crud/merge",
      "static_assets": "crud/merge",
    },
    doc_types: {
      "static_assets": "asset",
      "template_doc": "crdt",
      "content_doc": "crdt",
      "user_content_doc": "crdt",
      "collaborative_doc": "crdt",
      "submissions_doc": "crdt",
    },
  },
  viewer_template: {
    capabilities: {
      "template_doc": "crud/readonly",
      "content_doc": "crud/readonly",
      "collaborative_doc": "crud/merge",
      "submissions_doc": "crud/appendonly",
      "static_assets": "crud/readonly",
    },
    doc_types: {
      "static_assets": "asset",
      "template_doc": "crdt",
      "content_doc": "crdt",
      "collaborative_doc": "crdt",
      "submissions_doc": "crdt",
    },
    no_update_from_node: ["user_content_doc"],
    dont_send_to_node: ["user_content_doc"],
  },
};

// ========== TYPE EXPORTS ==========

export type FolderTemplate = typeof FOLDER_TEMPLATE;
export type ResourceTemplate = typeof RESOURCE_TEMPLATE;
