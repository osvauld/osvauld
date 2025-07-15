<script lang="ts">
	import Toast from "./Toast.svelte";
	import DeleteConfirmationModal from "./DeleteConfirmationModal.svelte";
	import Connector from "./Connector.svelte";
	import PasswordPromptModal from "../../common/PasswordPromptModal.svelte";
	import { uiState } from "../../state/";


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


{#if uiState.showConnector}
	<Connector onClose={handleConnectorClose} />
{/if}

{#if uiState.toastMessage.show}
	<div class="z-100">
		<Toast />
	</div>
{/if}
