/**
 * Scribe API - Frontend utilities for page/space management
 *
 * Uses new terminology:
 * - Space (was Folder/Website)
 * - Page (was Resource/Credential)
 *
 * This module provides typed wrappers for Tauri invoke calls.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { writeText, readText } from "@tauri-apps/plugin-clipboard-manager";

// ===== Types =====

export interface SpaceResponse {
  id: string;
  name: string;
  description: string;
  isDefault: boolean;
}

export interface PageMetadata {
  id: string;
  title: string;
  pageType: string;
  spaceId: string;
  lastModified: number;
  favourite: boolean;
  preview?: string;
}

export interface PageResponse {
  id: string;
  data: {
    template_doc: number[];
    content_doc: number[];
    user_content_doc: number[];
    collaborative_doc: number[];
    submissions_doc: number[];
    static_assets: Record<string, string>;
  };
  favourite: boolean;
  lastAccessed: number;
  spaceId: string;
}

export interface SovereignNode {
  nodeId: string;
  username: string;
  userPublicKey: string;
  devicePublicKey: string;
  isConnected: boolean;
  lastConnectedAt?: number;
}

export interface UserDetails {
  userId: string;
  deviceId: string;
  username: string;
  publicKey: string;
  deviceKey: string;
}

export interface LayerUpdate {
  pageId: string;
  layerName: string;
  update: Uint8Array;
}

// ===== Space API =====

/**
 * Create a new space
 */
export async function createSpace(
  name: string,
  description: string,
  spaceTemplateJson: string
): Promise<SpaceResponse> {
  return invoke("handle_create_space", {
    input: { name, description, spaceTemplateJson },
  });
}

/**
 * List all accessible spaces
 */
export async function listSpaces(): Promise<SpaceResponse[]> {
  return invoke("handle_list_spaces");
}

/**
 * Delete a space
 */
export async function deleteSpace(spaceId: string): Promise<void> {
  return invoke("handle_delete_space", { input: { spaceId } });
}

/**
 * Share a space with another user
 */
export async function shareSpace(
  spaceId: string,
  userId: string,
  recipientRole: string
): Promise<void> {
  return invoke("handle_share_space", {
    input: { spaceId, userId, recipientRole },
  });
}

// ===== Page API =====

/**
 * Create a new page in a space
 */
export async function createPage(
  spaceId: string,
  pageType: string,
  permitTemplateJson: string,
  metadataJson: string
): Promise<PageMetadata> {
  return invoke("handle_create_page", {
    input: { spaceId, pageType, permitTemplateJson, metadataJson },
  });
}

/**
 * Open a page and get its content
 * This also subscribes to Scribe updates for the page
 */
export async function openPage(pageId: string): Promise<PageResponse> {
  return invoke("handle_open_page", { input: { pageId } });
}

/**
 * Close a page and unsubscribe from updates
 */
export async function closePage(pageId: string): Promise<void> {
  return invoke("handle_close_page", { input: { pageId } });
}

/**
 * Apply a CRDT update to a page layer
 */
export async function applyUpdate(
  pageId: string,
  layerName: string,
  update: Uint8Array
): Promise<void> {
  return invoke("handle_apply_update", {
    input: { pageId, layerName, update: Array.from(update) },
  });
}

/**
 * List all accessible pages
 */
export async function listPages(): Promise<PageMetadata[]> {
  return invoke("handle_list_pages");
}

// ===== Auth API =====

/**
 * Check if user is signed up
 */
export async function checkSignupStatus(): Promise<{ isSignedUp: boolean }> {
  return invoke("check_signup_status");
}

/**
 * Get user details
 */
export async function getUserDetails(): Promise<UserDetails> {
  return invoke("get_user_details");
}

/**
 * Sign up with username and passphrase
 */
export async function signUp(
  username: string,
  passphrase: string
): Promise<{ username: string; deviceKey: string; encryptionKey: string; userId: string }> {
  return invoke("handle_sign_up", { input: { username, passphrase } });
}

/**
 * Check if private key is loaded
 */
export async function checkPrivateKeyLoaded(): Promise<boolean> {
  return invoke("check_private_key_loaded");
}

/**
 * Login with passphrase
 */
export async function login(passphrase: string): Promise<void> {
  return invoke("login", { input: { passphrase } });
}

/**
 * Logout
 */
export async function logout(): Promise<void> {
  return invoke("handle_logout");
}

/**
 * Get one-time permit for authentication
 */
export async function getOneTimePermit(): Promise<{ permit: string; permitPubKey: string }> {
  return invoke("get_one_time_permit");
}

// ===== User API =====

/**
 * Get known users (contacts)
 */
export async function getKnownUsers(): Promise<
  Array<{ userId: string; username: string; publicKey: string }>
> {
  return invoke("handle_get_known_users");
}

/**
 * Get sovereign nodes (paired Kunki nodes)
 */
export async function getSovereignNodes(): Promise<SovereignNode[]> {
  return invoke("handle_get_sovereign_nodes");
}

// ===== P2P API =====

/**
 * Start P2P listener
 */
export async function startP2PListener(): Promise<void> {
  return invoke("start_p2p_listener");
}

/**
 * Add a sovereign node connection
 */
export async function addSovereignNode(connectionString: string): Promise<void> {
  return invoke("handle_add_sovereign_node", { input: { connectionString } });
}

/**
 * Publish a space to a sovereign node
 */
export async function publishSpace(spaceId: string, nodeId: string): Promise<void> {
  return invoke("handle_publish_space", { spaceId, nodeId });
}

/**
 * Connect to a website via connection string (viewer mode)
 */
export async function connectToWebsite(connectionString: string): Promise<void> {
  return invoke("handle_connect_to_website", { input: connectionString });
}

// ===== Event Listeners =====

/**
 * Listen for page update events from Scribe
 */
export async function listenForPageUpdates(
  callback: (update: LayerUpdate) => void
): Promise<UnlistenFn> {
  return listen<LayerUpdate>("page-update", (event) => {
    callback(event.payload);
  });
}

/**
 * Listen for space sync events
 */
export async function listenForSpaceSync(
  callback: (payload: { spaceId: string; spaceName: string }) => void
): Promise<UnlistenFn> {
  return listen("space-synced", (event) => {
    callback(event.payload as { spaceId: string; spaceName: string });
  });
}

/**
 * Listen for page sync events
 */
export async function listenForPageSync(
  callback: (metadata: PageMetadata) => void
): Promise<UnlistenFn> {
  return listen("page-synced", (event) => {
    callback(event.payload as PageMetadata);
  });
}

// ===== Viewer Sync Event Listeners =====

export interface Space {
  id: string;
  name: string;
  parent_space_id: string | null;
  owner_did: string;
  is_default: boolean;
  description: string | null;
  created_at: number;
  updated_at: number;
}

export interface Page {
  id: string;
  space_id: string;
  name: string;
  page_type: string;
  owner_did: string;
  is_private: boolean;
  created_at: number;
  updated_at: number;
}

export interface ViewerSpaceReceivedPayload {
  nodeId: string;
  space: Space;
  pageCount: number;
}

export interface ViewerPageReceivedPayload {
  nodeId: string;
  page: Page;
  isLast: boolean;
}

/**
 * Listen for viewer space received events
 * Called when viewer receives space metadata from node
 */
export async function listenForViewerSpaceReceived(
  callback: (payload: ViewerSpaceReceivedPayload) => void
): Promise<UnlistenFn> {
  return listen<ViewerSpaceReceivedPayload>("viewer-space-received", (event) => {
    callback(event.payload);
  });
}

/**
 * Listen for viewer page received events
 * Called for each page as it's received from node
 */
export async function listenForViewerPageReceived(
  callback: (payload: ViewerPageReceivedPayload) => void
): Promise<UnlistenFn> {
  return listen<ViewerPageReceivedPayload>("viewer-page-received", (event) => {
    callback(event.payload);
  });
}

// ===== Legacy Compatibility =====

/**
 * Check if running in Tauri environment
 */
function isTauri(): boolean {
  // @ts-ignore
  return typeof window.__TAURI__ !== "undefined";
}

/**
 * Fallback function for browsers without Clipboard API support
 */
function fallbackCopyTextToClipboard(text: string): void {
  const textArea = document.createElement("textarea");
  textArea.value = text;
  textArea.style.top = "0";
  textArea.style.left = "0";
  textArea.style.position = "fixed";
  textArea.style.opacity = "0";

  document.body.appendChild(textArea);
  textArea.focus();
  textArea.select();

  try {
    const successful = document.execCommand("copy");
    if (!successful) {
      throw new Error("execCommand copy failed");
    }
  } catch (err) {
    console.error("Fallback: Could not copy text: ", err);
  }

  document.body.removeChild(textArea);
}

/**
 * Write text to clipboard
 */
export const writeToClipboard = async (text: string): Promise<void> => {
  if (isTauri()) {
    try {
      await writeText(text);
    } catch (error) {
      console.error("Error writing to clipboard:", error);
      throw error;
    }
  } else {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      try {
        await navigator.clipboard.writeText(text);
      } catch (error) {
        console.error("Clipboard API failed, falling back to execCommand:", error);
        fallbackCopyTextToClipboard(text);
      }
    } else {
      fallbackCopyTextToClipboard(text);
    }
  }
};

/**
 * Read text from clipboard
 */
export const readFromClipboard = async (): Promise<string> => {
  if (isTauri()) {
    try {
      return await readText();
    } catch (error) {
      console.error("Error reading from clipboard:", error);
      throw error;
    }
  } else {
    if (navigator.clipboard && navigator.clipboard.readText) {
      try {
        return await navigator.clipboard.readText();
      } catch (error) {
        console.error("Clipboard API readText failed:", error);
        throw new Error("Clipboard read failed - clipboard access may be denied or not supported");
      }
    } else {
      throw new Error("Clipboard read not supported - please upgrade your browser or copy manually");
    }
  }
};

/**
 * Export certificate for backup
 */
export async function exportCertificate(passphrase: string): Promise<{
  mnemonic: string;
  username: string;
}> {
  return invoke("handle_export_certificate", { input: { passphrase } });
}

/**
 * Change passphrase
 */
export async function changePassphrase(
  currentPassphrase: string,
  newPassphrase: string
): Promise<void> {
  return invoke("handle_change_passphrase", {
    input: { currentPassphrase, newPassphrase },
  });
}

/**
 * Add a device with mnemonic
 */
export async function addDevice(mnemonic: string, passphrase: string): Promise<void> {
  return invoke("handle_add_device", { input: { mnemonic, passphrase } });
}

