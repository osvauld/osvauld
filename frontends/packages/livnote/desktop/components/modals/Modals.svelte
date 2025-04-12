<script lang="ts">
	import Toast from "./Toast.svelte";
	import DeleteConfirmationModal from "./DeleteConfirmationModal.svelte";
	import Connector from "./Connector.svelte";
	import AddUserModal from "./AddUserModal.svelte";
	import {
		sendMessage,
		PasswordPromptModal,
	} from "@osvauld/password-manager-common";
	import { uiState } from "../../state/";

	const handleAddUser = async (event: any) => {
		try {
			const userResponse = await sendMessage("addKnownUser", event.detail);
			console.log("initiating first connection");
			const firstConnectionResponse = await sendMessage(
				"initiateFirstConnection",
				{
					user: userResponse.user,
					device: userResponse.device,
				},
			);
			console.log(firstConnectionResponse);
			uiState.showToast("User added successfully", true);
		} catch (error) {
			uiState.showToast("Failed to add user", false);
			console.error("Error adding user:", error);
		}
		uiState.toggleModal("showAddUser", false);
	};

	const handleConnectorClose = () => {
		uiState.toggleModal("showConnector", false);
	};
</script>

<!-- Each modal is conditionally rendered based on its state -->
{#if uiState.deleteConfirmationModal.show}
	<DeleteConfirmationModal />
{/if}

{#if uiState.passwordPromptModal.show}
	<PasswordPromptModal
		changePassword={uiState.passwordPromptModal.isChangePassword}
		onClose={() => uiState.hidePasswordPrompt()} />
{/if}

{#if uiState.showAddUser}
	<AddUserModal
		userAdd={handleAddUser}
		close={() => uiState.toggleModal("showAddUser", false)} />
{/if}

{#if uiState.showConnector}
	<Connector onClose={handleConnectorClose} />
{/if}

{#if uiState.toastMessage.show}
	<div class="z-100">
		<Toast />
	</div>
{/if}
