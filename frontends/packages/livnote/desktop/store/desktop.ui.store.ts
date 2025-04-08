import { writable } from "svelte/store";

export interface Vault {
	id: string;
	name: string;
	description?: string;
}

interface Toast {
	show: boolean;
	message: string;
	success: boolean;
}

interface DeleteConfirmation {
	item: string;
	show: boolean;
}

interface PasswordPrompt {
	isChangePassword: boolean;
	show: boolean;
}

export let language = writable<string>("en");
export let currentView = writable<string>("all");
export let vaults = writable<Vault[]>([]);
export let currentVault = writable<Vault>({ id: "all", name: "all vaults" });
export let notes = writable<any[]>([]);
export let selectedCategory = writable<string>("");
export let addCredentialModal = writable<boolean>(false);
export let selectedVaultForInput = writable<any>({});
export let viewCredentialModal = writable<boolean>(false);
export let currentNote = writable<any>({});
export let addDeviceModal = writable<boolean>(false);
export let refreshCredentialList = writable<boolean>(false);
export let refreshSidePanel = writable<boolean>(false);

export let deleteConfirmationModal = writable<DeleteConfirmation>({ item: "", show: false });
export let toastStore = writable<Toast>({ show: false, message: "", success: true });
export let showWelcome = writable<boolean>(true);
export let showSyncQr = writable<boolean>(false);

export let noteViewLayout = writable<boolean>(false);

export let showConnector = writable<boolean>(false);

export let noteId = writable<string>("");
export let showAddUser = writable<boolean>(false);

export let passwordPromptModal = writable<PasswordPrompt>({
	isChangePassword: false,
	show: false,
});
