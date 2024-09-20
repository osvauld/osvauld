import { writable } from "svelte/store";
import browser from "webextension-polyfill";
import { SelectedSection } from "../dtos/ui.dto";
export let selectedPage = writable("");
export let showAddFolderDrawer = writable(false);
export let showAddGroupDrawer = writable(false);
export let showFolderShareDrawer = writable(false);
export let showFolderRenameDrawer = writable(false);
export let showCredentialShareDrawer = writable(false);
export let showAddUserDrawer = writable(false);
export let allUsersSelected = writable(false);
export let showAddUserToGroupDrawer = writable(false);
export let showRenameGroupDrawer = writable(false);
export let showCredentialDetailsDrawer = writable(false);
export let credentialIdForEdit = writable(null);
export let buttonRef = writable(null);
export let showMoreOptions = writable(false);
export let modalManager = writable<MoreActions | null>(null);
export let DeleteConfirmationModal = writable(false);
export let accessListSelected = writable(false);
export let showMoreGroupOptions = writable(false);
export let toastStore = writable({ show: false, message: "", type: true });
export let selectedSection = writable<SelectedSection>("SharedFolders");
export let showAddCliDrawer = writable(false);
export let showAddEnvDrawer = writable(false);
export let promptPassword = writable(false);

const storedPage = browser.storage.local.get("selectedPage");
type MoreActionsTypes = "Folder" | "Group" | "Credential";
type MoreActions = {
	id: string;
	name: string;
	type: MoreActionsTypes;
	private?: boolean;
};
storedPage.then((value) => {
	if (value.selectedPage) {
		selectedPage.set(value.selectedPage);
	} else {
		selectedPage.set("Folders");
	}
});
selectedPage.subscribe((value) => {
	browser.storage.local.set({ selectedPage: value });
});
