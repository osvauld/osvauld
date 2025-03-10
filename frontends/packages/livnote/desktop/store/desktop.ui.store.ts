import { writable } from "svelte/store";
import { WSConnection } from "../components/connection/wsConnector.ts";

export let language = writable("en");
export let currentView = writable("all");
export let vaults = writable([]);
export let currentVault = writable({ id: "all", name: "all vaults" });
export let notes = writable([]);
export let selectedCategory = writable("");
export let addCredentialModal = writable(false);
export let selectedVaultForInput = writable({});
export let viewCredentialModal = writable(false);
export let currentNote = writable({});
export let addDeviceModal = writable(false);
export let refreshCredentialList = writable(false);

export let deleteConfirmationModal = writable({ item: "", show: false });
export let toastStore = writable({ show: false, message: "", success: true });
export let showWelcome = writable(true);
export let showSyncQr = writable(false);

export let noteViewLayout = writable(false);

export let showConnector = writable(false);

export let wsConnector = writable(new WSConnection());
export let noteId = writable("");
export let showAddUser = writable(false);
