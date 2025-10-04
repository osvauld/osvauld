<script lang="ts">
	import Toast from "./Toast.svelte";
	import DeleteConfirmationModal from "./DeleteConfirmationModal.svelte";
	import PasswordPromptModal from "../../common/PasswordPromptModal.svelte";
	import ConnectUserModal from "./ConnectUserModal.svelte";
	import { uiState } from "../../state/";
</script>

<!-- Each modal is conditionally rendered based on its state -->
{#if uiState.deleteConfirmationModal.show}
	<DeleteConfirmationModal />
{/if}

{#if uiState.passwordPromptModal.show}
	<PasswordPromptModal
		changePassword={uiState.passwordPromptModal.isChangePassword}
		onClose={() => uiState.hidePasswordPrompt()}
	/>
{/if}

{#if uiState.connectUserModal.show}
	<ConnectUserModal />
{/if}

{#if uiState.toastMessage.show}
	<div class="z-100">
		<Toast />
	</div>
{/if}
