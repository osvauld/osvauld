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

// ========== SYNC CONSENT TEMPLATES ==========
//
// Viewer-issued permits that express consent to receive sync updates.
// These permits are issued by the viewer back to the node after receiving
// initial space/page data.
//
// Flow:
// 1. Node sends SpaceSync with space + pages to viewer
// 2. Viewer stores space/pages locally
// 3. Viewer issues consent permits using these templates:
//    - 1 space consent permit (allows node to add new pages)
//    - N page consent permits (allows node to send layer updates)
// 4. Viewer sends permits back to node
// 5. Node stores permits keyed by (viewer_did, resource_id)
// 6. When syncing updates, node attaches the viewer-issued permit
// 7. Viewer verifies "this is a permit I issued" before accepting update
//
// Note: These permits use the node's original viewer permit as `prf` (proof)
// to derive permissions from the delegation chain.

// Viewer consent to sync a space + accept new pages
// Issued by: Viewer
// Audience: Node
// Proof: Node's original space_viewer permit
export const SYNC_SPACE_CONSENT_TEMPLATE = {
  consent_template: {
    token_type: "sync_space_consent",
    // Operations the viewer consents to receive
    operations: {
      "receive_pages": "allow",    // Consent to receive new pages in this space
      "receive_updates": "allow",  // Consent to receive updates for this space
    },
    // Authorization capabilities for sync
    auth_capabilities: {
      accept_sync: true,           // Viewer consents to accept sync updates
      accept_new_pages: true,      // Viewer consents to receive new pages
    },
    relationship: "sync_consent",
    // CEL rules for validation
    cel_rules: {
      // Is this a valid sync consent permit?
      is_sync_consent: "token_type == 'sync_space_consent' && relationship == 'sync_consent'",
      // Can accept sync updates?
      can_accept_sync: "auth_capabilities.accept_sync == true",
      // Can accept new pages?
      can_accept_pages: "auth_capabilities.accept_new_pages == true",
    },
    // Two-permit functions for node to use when syncing
    functions: {
      // Node uses this to verify it can send sync to this viewer
      can_send_sync: "self.token_type == 'sync_space_consent' && self.space_id == context.space_id",
      // Node uses this to verify it can send new pages to this viewer
      can_send_new_page: "self.auth_capabilities.accept_new_pages == true && self.space_id == context.space_id",
    },
  },
};

// Viewer consent to sync a specific page's layers
// Issued by: Viewer
// Audience: Node
// Proof: Node's original page_viewer permit
export const SYNC_PAGE_CONSENT_TEMPLATE = {
  consent_template: {
    token_type: "sync_page_consent",
    // Operations the viewer consents to receive
    operations: {
      "receive_layer_updates": "allow",  // Consent to receive layer updates for this page
    },
    // Layers the viewer consents to receive updates for
    // This mirrors the layers from the original viewer permit
    layers: {
      "template_doc": {
        capability: "receive",
        type: "crdt",
      },
      "content_doc": {
        capability: "receive",
        type: "crdt",
      },
      "collaborative_doc": {
        capability: "receive",
        type: "crdt",
      },
      "submissions_doc": {
        capability: "receive",
        type: "crdt",
      },
      "static_assets": {
        capability: "receive",
        type: "asset",
      },
    },
    // Sync rules for which layers to accept updates
    sync: {
      no_incoming_updates: ["submissions_doc"],  // Viewer doesn't accept incoming for submissions
    },
    // Authorization capabilities
    auth_capabilities: {
      accept_sync: true,
    },
    relationship: "sync_consent",
    // CEL rules for validation
    cel_rules: {
      // Is this a valid page sync consent permit?
      is_sync_consent: "token_type == 'sync_page_consent' && relationship == 'sync_consent'",
      // Can accept layer updates?
      can_accept_layer: "has(layers[context.layer]) && layers[context.layer].capability == 'receive'",
    },
    // Two-permit functions for node to use when syncing
    functions: {
      // Node uses this to verify it can send layer update to this viewer
      can_send_layer: "self.token_type == 'sync_page_consent' && self.page_id == context.page_id && has(self.layers[context.layer])",
      // Viewer uses this to verify the permit is one they issued
      is_my_consent: "self.iss == context.our_pubkey && self.aud == context.their_pubkey",
    },
  },
};

// ========== TYPE EXPORTS ==========

export type SpaceTemplate = typeof SPACE_TEMPLATE;
export type PageTemplate = typeof PAGE_TEMPLATE;
export type SyncSpaceConsentTemplate = typeof SYNC_SPACE_CONSENT_TEMPLATE;
export type SyncPageConsentTemplate = typeof SYNC_PAGE_CONSENT_TEMPLATE;
