<script lang="ts">
	import CollaboratorSelector from "./CollaboratorSelector.svelte";
	import { sendMessage } from "../../utils/helper";
	import { dataState, uiState } from "../../state/";

	// Define interfaces
	interface Collaborator {
		username: string;
		online: boolean;
		id?: string;
		publicKey?: string;
	}

	// Props
	interface Props {
		showShareList?: boolean;
	}

	let { showShareList = $bindable(false) }: Props = $props();

	// Local state
	let availableCollaborators = $state<Collaborator[]>([]);
	let existingCollaborators = $state<Collaborator[]>([]);

	async function fetchUsers() {
		try {
			const users = await sendMessage("getKnownUsers");
			availableCollaborators = users;

			if (dataState.currentNoteId) {
				existingCollaborators = dataState.sharedUsers;
			}
		} catch (error) {
			console.error("Error fetching users:", error);
			uiState.showToast("Failed to load users", false);
		}
	}

	const handleShareNote = async (
		selectedUsers: { username: string; id: string }[],
	) => {
		if (selectedUsers.length === 0) return;

		const user = selectedUsers[0];
		try {
			const abilities = ["crud/read", "crud/update", "ucan/share"];
			const resourceURI = `livnote:resource:${dataState.currentNoteId}`;
			const permissionsToGrant = abilities.map((ability) => [
				resourceURI,
				ability,
			]);
			await sendMessage("shareResource", {
				resourceId: dataState.currentNoteId,
				userId: user.id,
				permissions: permissionsToGrant,
			});

			// Show success toast
			uiState.showToast("Note shared successfully", true);

			// Close the share panel
			showShareList = false;
		} catch (error) {
			console.error("Error sharing note:", error);
			uiState.showToast("Failed to share note", false);
		}
	};

	// Initialize data when component is shown
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
		title="Invite to edit"
		availableUsers={availableCollaborators}
		existingUsers={existingCollaborators}
		maxSelections={1}
		buttonText="Add to collaborate"
		variant="note"
		onShare={handleShareNote}
		onClose={() => (showShareList = false)}
	/>
{/if}
