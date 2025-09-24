<script lang="ts">
	// ⚠️  TEMPORARY CHANGES FOR UI TESTING - Contains placeholder data ⚠️
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

	// Local state - TEMPORARY: Using placeholder data for UI testing
	// let availableCollaborators = $state<Collaborator[]>([
	// 	// Online users with various states
	// 	{
	// 		username: "sarah.chen",
	// 		online: true,
	// 		id: "user-001",
	// 		publicKey: "pk-001",
	// 	},
	// 	{
	// 		username: "alex.rodriguez",
	// 		online: true,
	// 		id: "user-002",
	// 		publicKey: "pk-002",
	// 	},
	// 	{
	// 		username: "emma.wilson",
	// 		online: true,
	// 		id: "user-003",
	// 		publicKey: "pk-003",
	// 	},
	// 	{
	// 		username: "michael.brown",
	// 		online: true,
	// 		id: "user-004",
	// 		publicKey: "pk-004",
	// 	},

	// 	// Offline users
	// 	{
	// 		username: "david.kim",
	// 		online: false,
	// 		id: "user-005",
	// 		publicKey: "pk-005",
	// 	},
	// 	{
	// 		username: "lisa.thompson",
	// 		online: false,
	// 		id: "user-006",
	// 		publicKey: "pk-006",
	// 	},
	// 	{
	// 		username: "james.garcia",
	// 		online: false,
	// 		id: "user-007",
	// 		publicKey: "pk-007",
	// 	},
	// 	{
	// 		username: "rachel.martinez",
	// 		online: false,
	// 		id: "user-008",
	// 		publicKey: "pk-008",
	// 	},

	// 	// More users for testing search functionality
	// 	{
	// 		username: "tom.anderson",
	// 		online: true,
	// 		id: "user-009",
	// 		publicKey: "pk-009",
	// 	},
	// 	{
	// 		username: "sophie.clark",
	// 		online: false,
	// 		id: "user-010",
	// 		publicKey: "pk-010",
	// 	},
	// 	{
	// 		username: "ryan.taylor",
	// 		online: true,
	// 		id: "user-011",
	// 		publicKey: "pk-011",
	// 	},
	// 	{
	// 		username: "maria.gonzalez",
	// 		online: false,
	// 		id: "user-012",
	// 		publicKey: "pk-012",
	// 	},
	// 	{
	// 		username: "kevin.lee",
	// 		online: true,
	// 		id: "user-013",
	// 		publicKey: "pk-013",
	// 	},
	// 	{
	// 		username: "anna.johnson",
	// 		online: false,
	// 		id: "user-014",
	// 		publicKey: "pk-014",
	// 	},
	// 	{
	// 		username: "chris.davis",
	// 		online: true,
	// 		id: "user-015",
	// 		publicKey: "pk-015",
	// 	},
	// ]);

	let availableCollaborators = $state<Collaborator[]>([
		// Current collaborators with mixed online/offline status
		{
			username: "sarah.chen",
			online: true,
			id: "user-001",
			publicKey: "pk-001",
		},
	]);
	let existingCollaborators = $state<Collaborator[]>([
		// Current collaborators with mixed online/offline status
	]);

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
			// TEMPORARY: Commented out for UI testing with placeholder data
			// fetchUsers();
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
		onShare={handleShareNote}
		onClose={() => (showShareList = false)}
	/>
{/if}
