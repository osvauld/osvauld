import { invoke } from "@tauri-apps/api/core";
import { writeText, readText } from '@tauri-apps/plugin-clipboard-manager';

// ===== Backend Types =====

// Folder Types
export interface AddFolderInput {
  name: string;
  description: string;
}

export interface FolderResponse {
  id: string;
  name: string;
  description: string;
  default: boolean;
}

// Resource Types
export interface AddResourceInput {
  resourcePayload: string;
  folderId: string;
  resourceType: string;
  ucanTemplateJson: string;
  metadataJson: string;
}

export interface ResourceMetadata {
  id: string;
  title: string;
  resourceType: string;
  folderId: string;
  lastModified: number;
  favourite: boolean;
  preview?: string;
}

export interface GetResourceInput {
  resourceId: string;
}

export interface ResourceResponse {
  id: string;
  data: {
    template_doc: number[];
    content_doc: number[];
    user_content_doc: number[];
    collaborative_doc: number[];
    submissions_doc: number[];
    static_assets: Array<{ id: string; data: number[] }>;
  };
  favourite: boolean;
  lastAccessed: number;
  folderId: string;
}

/**
 * Send message to Tauri backend
 * Follows livnote's sendMessage pattern with handler map
 */
export const sendMessage = async (action: string, data?: any): Promise<any> => {
  try {
    // Map actions to their specific handlers
    // ✅ = Currently used, ⏳ = TODO/Not implemented, ❌ = To be removed
    const handlerMap = {
      // Auth handlers (core - always keep)
      isSignedUp: () => invoke("check_signup_status"), // ✅
      savePassphrase: (data: any) => invoke("handle_sign_up", { input: data }), // ✅
      checkPvtLoaded: () => invoke("check_private_key_loaded"), // ✅
      login: (data: any) => invoke("login", { input: data }), // ✅
      logout: () => invoke("handle_logout"), // ✅
      addDevice: (data: any) => invoke("handle_add_device", { input: data }), // ✅
      exportCertificate: (data: any) => invoke("handle_export_certificate", { input: data }), // ✅
      changePassphrase: (data: any) => invoke("handle_change_passphrase", { input: data }), // ✅
      getUserDetails: () => invoke('get_user_details'), // ✅
      getOneTimeUcanToken: () => invoke('get_one_time_ucan_token'), // ✅

      // Folder handlers
      addFolder: (data: any) => invoke("handle_add_folder", { input: data }), // ✅ data.svelte.ts:164
      getFolder: () => invoke("handle_get_folders"), // ✅ data.svelte.ts:119
      deleteFolder: (data: any) => invoke("handle_soft_delete_folder", { input: data }), // ⏳ TODO
      shareFolder: (data: any) => invoke("handle_share_folder", { input: data }), // ⏳ TODO
      getSharedFolderUsers: (data: any) => invoke("handle_get_shared_folder_users", { input: data }), // ⏳ TODO

      // Resource handlers
      addCredential: (data: any) => invoke("handle_add_resource", { input: data }), // ✅ data.svelte.ts:252
      getCredential: (data: any) => invoke("handle_get_resource", { input: data }), // ✅ data.svelte.ts:326
      getAllResourcesMetadata: () => invoke("handle_get_all_resources_metadata"), // ✅ data.svelte.ts:133
      updateCredential: (data: any) => invoke("handle_update_resource", { input: data }), // ⏳ data.svelte.ts:457 (used but not implemented)
      getCredentialsForFolder: (data: any) => invoke("handle_get_resources_for_folder", { input: data }), // ⏳ TODO
      deleteResource: (data: any) => invoke("soft_delete_resource", { input: data }), // ⏳ TODO
      shareResource: (data: any) => invoke("handle_share_resource", { input: data }), // ⏳ TODO
      searchResource: (data: any) => invoke("handle_search_resources", { input: data }), // ⏳ TODO
      toggleFav: (data: any) => invoke("handle_toggle_fav", { input: data }), // ⏳ TODO
      updateLastAccessed: (data: any) => invoke("handle_update_last_accessed", { input: data }), // ⏳ TODO

      // User handlers
      addKnownUser: (data: any) => invoke("handle_add_user", { input: data }), // ⏳ TODO
      getKnownUsers: () => invoke("handle_get_known_users"), // ⏳ TODO

      // Network/P2P handlers
      startP2PListener: () => invoke("start_p2p_listener"), // ✅ Initialize P2P network
      addSovereignNode: (data: any) => invoke("handle_add_sovereign_node", { input: data }), // ✅ Add sovereign node connection
      updateCurrentNote: (data: any) => { invoke('update_current_note', { input: data }) }, // ❌ Old live edit - to be removed
      publishResource: (data: any) => invoke("handle_publish_resource", { resourceId: data.resourceId }), // ⏳ TODO (Phase 3)
      connectToWebsite: (data: any) => invoke("handle_connect_to_website", { input: data.connectionString }), // ✅ ACTIVE - Viewer connection
      syncResource: (data: any) => invoke("handle_sync_resource", { input: data }), // ✅ P2P resource sync
      folderSyncViewer: (data: any) => invoke("handle_folder_sync_viewer", { folderId: data.folderId }), // ❌ Website feature - removed
    };
    //@ts-ignore
    const handler = handlerMap[action];
    if (!handler) {
      throw new Error(`Unknown action: ${action}`);
    }

    return await handler(data);
  } catch (error) {
    console.error("Error invoking Tauri command:", error);
    throw error;
  }
};

/**
 * Write text to clipboard
 */
export const writeToClipboard = async (text: string) => {
  const tauriEnv = isTauri();
  if (tauriEnv) {
    try {
      await writeText(text);
    } catch (error) {
      console.error("Error writing to clipboard:", error);
    }
  } else {
    // Check if modern Clipboard API is available
    if (navigator.clipboard && navigator.clipboard.writeText) {
      try {
        await navigator.clipboard.writeText(text);
      } catch (error) {
        console.error("Clipboard API failed, falling back to execCommand:", error);
        fallbackCopyTextToClipboard(text);
      }
    } else {
      // Fallback for older browsers
      fallbackCopyTextToClipboard(text);
    }
  }
};

// Fallback function for browsers without Clipboard API support
function fallbackCopyTextToClipboard(text: string) {
  const textArea = document.createElement("textarea");
  textArea.value = text;

  // Avoid scrolling to bottom
  textArea.style.top = "0";
  textArea.style.left = "0";
  textArea.style.position = "fixed";
  textArea.style.opacity = "0";

  document.body.appendChild(textArea);
  textArea.focus();
  textArea.select();

  try {
    const successful = document.execCommand('copy');
    if (!successful) {
      throw new Error('execCommand copy failed');
    }
  } catch (err) {
    console.error('Fallback: Could not copy text: ', err);
  }

  document.body.removeChild(textArea);
}

/**
 * Read text from clipboard
 */
export const readFromClipboard = async (): Promise<string> => {
  const tauriEnv = isTauri();
  if (tauriEnv) {
    try {
      return await readText();
    } catch (error) {
      console.error("Error reading from clipboard:", error);
      throw error;
    }
  } else {
    // Check if modern Clipboard API is available
    if (navigator.clipboard && navigator.clipboard.readText) {
      try {
        return await navigator.clipboard.readText();
      } catch (error) {
        console.error("Clipboard API readText failed:", error);
        throw new Error("Clipboard read failed - clipboard access may be denied or not supported");
      }
    } else {
      // No fallback for readText in older browsers - it's not securely possible
      throw new Error("Clipboard read not supported - please upgrade your browser or copy manually");
    }
  }
};

function isTauri() {
  // @ts-ignore
  return typeof window.__TAURI__ !== "undefined";
}

/**
 * Get user details from backend
 */
export async function getUserDetails() {
  // For POC, return mock data until backend is ready
  // TODO: Replace with: return await sendMessage('getUserDetails');

  // Create a proper base64 encoded deviceId for clientId generation
  // This encodes a 32-byte random string
  const mockDeviceBytes = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    mockDeviceBytes[i] = Math.floor(Math.random() * 256);
  }
  const mockDeviceId = btoa(String.fromCharCode(...mockDeviceBytes));

  return {
    userId: "user-123",
    deviceId: mockDeviceId,
    username: "DemoUser",
    publicKey: "mock-public-key",
    deviceKey: "mock-device-key"
  };
}
