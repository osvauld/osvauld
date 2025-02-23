import { writable } from "svelte/store";
import { WSConnection } from "../components/lib/utils/wsConnector";

export let language = writable("en");
export let currentView = writable("all");
export let vaults = writable([]);
export let currentVault = writable({ id: "all", name: "all vaults" });
export let selectedCategory = writable("");
export let addCredentialModal = writable(false);
export let credentialEditorModal = writable(false);
export let selectedCategoryForInput = writable("");
export let selectedVaultForInput = writable({});
export let viewCredentialModal = writable(false);
export let currentCredential = writable({});
export let addDeviceModal = writable(false);
export let refreshCredentialList = writable(false);

export let deleteConfirmationModal = writable({ item: "", show: false });
export let toastStore = writable({ show: false, message: "", success: true });
export let showWelcome1 = writable(true);
export let showSyncQr = writable(false);

export let noteViewLayout = writable(false);

export let showConnector = writable(false);

export let wsConnector = writable(new WSConnection());
