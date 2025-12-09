/**
 * API layer for Tauri backend communication
 *
 * Generic sendMessage function that maps action names to Tauri invoke calls.
 */

import { invoke } from "@tauri-apps/api/core";

export const sendMessage = async (action: string, data?: any): Promise<any> => {
  const handlerMap: Record<string, (data?: any) => Promise<any>> = {
    // Auth handlers
    isSignedUp: () => invoke("check_signup_status"),
    savePassphrase: (d: any) => invoke("handle_sign_up", { input: d }),
    checkPvtLoaded: () => invoke("check_private_key_loaded"),
    login: (d: any) => invoke("login", { input: d }),
    logout: () => invoke("handle_logout"),
    addDevice: (d: any) => invoke("handle_add_device", { input: d }),
    exportCertificate: (d: any) => invoke("handle_export_certificate", { input: d }),
    changePassphrase: (d: any) => invoke("handle_change_passphrase", { input: d }),
    getUserDetails: () => invoke("get_user_details"),
    getOneTimePermit: () => invoke("get_one_time_permit"),

    // Space handlers (was Folder)
    addFolder: (d: any) => invoke("handle_create_space", { input: { name: d.name, description: d.description, spaceTemplateJson: "{}" } }),
    getFolder: () => invoke("handle_list_spaces"),
    deleteFolder: (d: any) => invoke("handle_delete_space", { input: { spaceId: d.folderId } }),
    shareFolder: (d: any) => invoke("handle_share_space", { input: { spaceId: d.folderId, userId: d.userId, recipientRole: d.role || "viewer" } }),

    // Page handlers (was Resource)
    addCredential: (d: any) => invoke("handle_create_page", { input: { spaceId: d.folderId, pageType: d.resourceType, permitTemplateJson: d.permitTemplateJson, metadataJson: d.metadataJson } }),
    getCredential: (d: any) => invoke("handle_open_page", { input: { pageId: d.resourceId } }),
    getAllResourcesMetadata: () => invoke("handle_list_pages"),
    updateCredential: (d: any) => invoke("handle_apply_update", { input: { pageId: d.resourceId, layerName: d.layerName || "content_doc", update: d.update } }),

    // User handlers
    getKnownUsers: () => invoke("handle_get_known_users"),
    getSovereignNodes: () => invoke("handle_get_sovereign_nodes"),

    // P2P handlers
    startP2PListener: () => invoke("start_p2p_listener"),
    addSovereignNode: (d: any) => invoke("handle_add_sovereign_node", { input: d }),
    publishFolder: (d: any) => invoke("handle_publish_space", { spaceId: d.folderId, nodeId: d.nodeId }),
    publishSpace: (d: any) => invoke("handle_publish_space", { spaceId: d.spaceId, nodeId: d.nodeId }),
    connectToWebsite: (d: any) => invoke("handle_connect_to_website", { input: d.connectionString }),
    getShareLink: (d: any) => invoke("handle_get_share_link", { spaceId: d.spaceId, nodeId: d.nodeId }),
  };

  const handler = handlerMap[action];
  if (!handler) {
    throw new Error(`Unknown action: ${action}`);
  }
  return handler(data);
};
