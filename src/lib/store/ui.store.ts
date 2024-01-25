import { writable } from "svelte/store";
export let selectedPage = writable("Credentials");
export let isLoggedIn = writable(false);
export let isSignedUp = writable(false);
export let showAddCredentialDrawer = writable(false);
export let showAddFolderDrawer = writable(false);
export let showAddGroupDrawer = writable(false);
export let showFolderShareDrawer = writable(false);
export let showCredentialShareDrawer = writable(false)
export let showAddUserDrawer = writable(false);
export let showAddUserToGroupDrawer = writable(false);