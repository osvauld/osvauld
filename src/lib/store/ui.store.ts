import { writable } from "svelte/store";
export let selectedPage = writable("Folders");
export let isLoggedIn = writable(false);
export let isSignedUp = writable(false);
export let showCredentialEditor = writable(false);
export let showAddFolderDrawer = writable(false);
export let showAddGroupDrawer = writable(false);
export let showFolderShareDrawer = writable(false);
export let showCredentialShareDrawer = writable(false)
export let showAddUserDrawer = writable(false);
export let allUsersSelected = writable(false);
export let adminStatus = writable(true);
export let showAddUserToGroupDrawer = writable(false);
export let showCredentialDetailsDrawer = writable(false);
export let showEditCredentialDialog = writable(false);
export let credentialIdForEdit = writable(null);
export const editPermissionTrigger = writable(false);
export const isPermissionChanged = writable(false);
export const accessSelectorIdentifier = writable(null);
