// Permissions Configuration - Permit Facts-Only Architecture
// Defines capability templates for spaces and pages
//
// IMPORTANT: This is the source of truth for permissions in Permit tokens.
// All authorization is stored in token facts - no URI capabilities are used.
//
// Terminology:
// - Space: Container for Pages (like a project or workspace)
// - Page: A sub-application instance with its own template
// - Layer: CRDT data containers (the actual collaborative state)
//
// Facts-Only Architecture:
// - All permissions stored in facts.operations, facts.layers, facts.auth_capabilities
// - CEL rules in facts.cel_rules provide single-permit authorization checks
// - CEL functions in facts.functions provide two-permit comparison logic
// - Relationships (owner/node/viewer) stored in facts.relationship
//
// Two-Permit CEL Functions (facts.functions):
// - `self.*` - Our permit's facts (+ iss, aud)
// - `peer.*` - Peer's permit's facts (+ iss, aud)
// - `context.*` - Identity context (our_pubkey, their_pubkey, layer, page_id, space_id)
//
// Backend transforms these templates into Permit token facts.

// ========== SPACE TEMPLATES ==========
//
// Space operations (no layer-based capabilities, only operations on the space)

export const SPACE_TEMPLATE = {
  owner_template: {
    // Operations available on this space (owner has all of them)
    operations: {
      "own": "allow",
      "get_share_link": "allow",
      "add_pages": "allow",
      "share_space": "allow",
    },
    delegation: {
      // What operations can be delegated to a node
      node: {
        token_type: "space_share",  // Data-driven: backend uses this for delegated token
        operations: {
          "get_share_link": "allow",
          "add_pages": "allow",
          "share_space": "allow",
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
        // Two-permit comparison functions (self vs peer)
        functions: {
          // Should we accept this viewer connection?
          should_accept_connection: "self.relationship == 'node' && peer.relationship == 'viewer' && peer.space_id == self.space_id",
          // Should we persist this viewer's share record?
          should_persist_share: "self.auth_capabilities.persist_share == true && peer.relationship == 'viewer'",
        },
      },
      // What operations can be delegated to a viewer
      viewer: {
        token_type: "space_viewer",  // Data-driven: backend uses this for delegated token
        operations: {
          "request_pages": "allow",
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
        // Two-permit comparison functions (self vs peer)
        functions: {
          // Can we request pages from this node?
          can_request_pages: "self.relationship == 'viewer' && peer.relationship == 'node'",
        },
      },
    },
  },
};

// ========== PAGE TEMPLATES ==========
//
// Page layers and their capabilities across different roles

export const PAGE_TEMPLATE = {
  owner_template: {
    // Operations on the page itself
    operations: {
      "own": "allow",
      "share_page": "allow",  // Permission to share this page with others
    },
    // Layers in this page (map format: layer_name -> {capability, type})
    layers: {
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
        token_type: "page_share",  // Data-driven: backend uses this for delegated token
        operations: {
          "share_page": "allow",  // Node can also share pages
        },
        layers: {
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
        // Two-permit comparison functions for sync decisions
        functions: {
          // Should we send this layer to the peer?
          should_send_layer: "self.page_id == peer.page_id && has(self.layers[context.layer]) && has(peer.layers[context.layer]) && self.layers[context.layer].capability == 'collaborator'",
          // Can we receive updates for this layer from peer?
          can_receive_layer: "self.page_id == peer.page_id && has(self.layers[context.layer]) && peer.layers[context.layer].capability in ['collaborator', 'submitter']",
          // Should we persist this viewer's share?
          should_persist_share: "self.auth_capabilities.persist_share == true && peer.relationship == 'viewer'",
        },
      },
      // What viewers receive
      viewer: {
        token_type: "page_viewer",  // Data-driven: backend uses this for delegated token
        operations: {},  // Viewers have no page-level operations
        layers: {
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
        // Two-permit comparison functions for sync decisions
        functions: {
          // Should we send this layer to the node? (viewer submitting data)
          should_send_layer: "self.page_id == peer.page_id && has(self.layers[context.layer]) && self.layers[context.layer].capability in ['collaborator', 'submitter']",
          // Can we receive updates for this layer from node?
          can_receive_layer: "self.page_id == peer.page_id && has(self.layers[context.layer]) && self.layers[context.layer].capability in ['viewer', 'collaborator']",
        },
      },
    },
  },
};

// ========== TYPE EXPORTS ==========

export type SpaceTemplate = typeof SPACE_TEMPLATE;
export type PageTemplate = typeof PAGE_TEMPLATE;
