<script lang="ts">
	import CollaboratorSelector from "./CollaboratorSelector.svelte";
	import { sendMessage } from "../../utils/helper";
	import { dataState, uiState } from "../../state/";

	interface Collaborator {
		username: string;
		online: boolean;
		id?: string;
		publicKey?: string;
	}

	interface Props {
		showShareList?: boolean;
	}

	let { showShareList = $bindable(false) }: Props = $props();

	let availableCollaborators = $state<Collaborator[]>([]);
	let existingCollaborators = $state<Collaborator[]>([]);

	async function fetchUsers() {
		try {
			const users = await sendMessage("getKnownUsers");
			availableCollaborators = users;
			existingCollaborators = dataState.sharedFolderUsers;
		} catch (error) {
			console.error("Error fetching users:", error);
			uiState.showToast("Failed to load users", false);
		}
	}

	const handleShareFolder = async (
		selectedUsers: { username: string; id: string }[],
	) => {
		if (selectedUsers.length === 0) return;

		const user = selectedUsers[0];
		try {
			let permissions = generateFolderPermissions(dataState.currentVault.id);
			await sendMessage("shareFolder", {
				folderId: dataState.currentVault.id,
				userId: user.id,
				permissions,
			});

			uiState.showToast("Folder shared successfully", true);
			showShareList = false;
			// Refresh shared folder users
			dataState.fetchSharedFolderUsers(dataState.currentVault.id);
		} catch (error) {
			console.error("Error sharing folder:", error);
			uiState.showToast("Failed to share folder", false);
		}
	};
	const generateFolderPermissions = (
		folderId: String,
		capabilityPrefix = "livnote",
	) => {
		const abilities = [
			"crud/read",
			"crud/update",
			"crud/delete",
			"add_resources",
			"share_folder",
		];
		const folderURI = `${capabilityPrefix}:folder:${folderId}`;
		const permissionsToGrant = abilities.map((ability) => [folderURI, ability]);

		return permissionsToGrant;
	};
	$effect(() => {
		if (showShareList) {
			fetchUsers();
		}
	});
</script>

{#if showShareList}
	<div
		class="bg-transparent fixed inset-0"
		role="presentation"
		aria-hidden="true"
		onclick={(e) => {
			e.stopPropagation();
			showShareList = false;
		}}
	></div>
	<CollaboratorSelector
		show={showShareList}
		title="Share folder"
		availableUsers={availableCollaborators}
		existingUsers={existingCollaborators}
		maxSelections={1}
		buttonText="Share folder"
		variant="folder"
		onShare={handleShareFolder}
		onClose={() => (showShareList = false)}
	/>
{/if}
