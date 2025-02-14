import { writable } from "svelte/store";
import { type Folder } from "@osvauld/password-manager-common/dtos/folder.dto";

export let currentVault = writable<Folder>({ id: "all", name: "All Vaults" });
export let vaultSwitchActive = writable(false);
export let selectedCredentialType = writable("");
export let categorySelection = writable(false);
export let vaults = writable([{ id: "all", name: "All Vaults" }]);
export let currentLayout = writable("home");
export let credentialLayoutType = writable("addition");
export let selectedCredential = writable({});
export let bottomNavActive = writable(true);
export let credentialListWithType = writable("");
export let refreshVaults = writable(false);
export let refreshCredentialList = writable(false);
export let vaultSwitchForAddingCredential = writable(false);
export let deleteConfirmationModal = writable({ item: "", show: false });
export let favoriteCredentials = writable(false);
