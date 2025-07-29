
import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";


export const getLastModifiedDate = (timestamp: number | undefined): string => {
  if (!timestamp) return "Not available";
  const date = new Date(timestamp);
  return date.toLocaleDateString() + " " + date.toLocaleTimeString();
};

export const sendMessage = async (action: string, data?: any): Promise<any> => {
  try {
    // Map actions to their specific handlers
    const handlerMap = {
      isSignedUp: () => invoke("check_signup_status"),
      savePassphrase: (data: any) => invoke("handle_sign_up", { input: data }),
      checkPvtLoaded: () => invoke("check_private_key_loaded"),
      login: (data: any) => invoke("login", { input: data }),
      addCredential: (data: any) =>
        invoke("handle_add_resource", { input: data }),
      addDevice: (data: any) => invoke("handle_add_device", { input: data }),
      exportCertificate: (data: any) =>
        invoke("handle_export_certificate", { input: data }),
      changePassphrase: (data: any) =>
        invoke("handle_change_passphrase", { input: data }),
      addFolder: (data: any) => invoke("handle_add_folder", { input: data }),
      getFolder: () => invoke("handle_get_folders"),
      getCredentialsForFolder: (data: any) =>
        invoke("handle_get_resources_for_folder", { input: data }),
      startP2PListner: () => invoke("start_p2p_listener"),
      deleteResource: (data: any) =>
        invoke("soft_delete_resource", { input: data }),
      deleteFolder: (data: any) =>
        invoke("handle_soft_delete_folder", { input: data }),
      toggleFav: (data: any) => invoke("handle_toggle_fav", { input: data }),
      updateLastAccessed: (data: any) =>
        invoke("handle_update_last_accessed", { input: data }),
      getAllCredentials: () =>
        invoke("handle_get_all_resources"),
      emitAllResources: (selectedResourceId?: string) =>
        invoke("emit_all_resources", {
          selectedResourceId: selectedResourceId ?? null
        }),
      logout: () => invoke("handle_logout"),
      sendSnapshot: (data: any) => invoke("send_snapshot", { snapshot: data }),
      updateCredential: (data: any) =>
        invoke("handle_update_resource", { input: data }),
      getCredential: (data: any) => invoke("handle_get_resource", { input: data }),
      addKnownUser: (data: any) =>
        invoke("handle_add_user", { input: data }),
      getKnownUsers: () => invoke("handle_get_known_users"),
      shareResource: (data: any) =>
        invoke("handle_share_resource", { input: data }),
      updateCurrentNote: (data: any) => {
        invoke('update_current_note', { input: data })
      },
      getUserDetails: () => invoke('get_user_details'),
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

export const writeToClipboard = async (text: string) => {
  const tauriEnv = isTauri();
  if (tauriEnv) {
    try {
      await writeText(text);
    } catch (error) {
      console.error("Error writing to clipboard:", error);
    }
  } else {
    navigator.clipboard.writeText(text);
  }
};

function isTauri() {
  // @ts-ignore
  return typeof window.__TAURI__ !== "undefined";
}
