<script lang="ts">
	import { dataState, uiState } from "../../state/";
	import { UserIcon, CheckVerified } from "../../icons";

	interface User {
		id: string;
		username: string;
		publicKey: string;
		isOnline: boolean;
	}

	const selectUser = (user: User) => {
		dataState.createChatWithUser(user.id, user.username);
	};

	const cancel = () => {
		uiState.hideUserSelectionModal();
	};
</script>

{#if uiState.userSelectionModal.show}
	<div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
		<div class="bg-osvauld-frameblack border border-osvauld-borderColor rounded-lg p-6 w-96 max-h-96 overflow-hidden">
			<div class="flex justify-between items-center mb-4">
				<h2 class="text-osvauld-fieldText font-medium text-lg">Select User to Chat With</h2>
				<button
					onclick={cancel}
					class="text-osvauld-fieldText opacity-60 hover:opacity-100"
				>
					×
				</button>
			</div>

			<div class="space-y-2 max-h-64 overflow-y-auto">
				{#each uiState.userSelectionModal.users as user (user.id)}
					<button
						onclick={() => selectUser(user)}
						class="w-full p-3 bg-osvauld-ninjablack border border-osvauld-borderColor rounded-lg hover:border-livnotelavender transition-colors duration-200 flex items-center space-x-3"
					>
						<div class="flex-shrink-0">
							<UserIcon color="currentColor" />
						</div>
						<div class="flex-1 text-left">
							<div class="flex items-center space-x-2">
								<span class="text-osvauld-fieldText font-medium">{user.username}</span>
								{#if user.isOnline}
									<div class="w-2 h-2 bg-green-500 rounded-full"></div>
								{/if}
							</div>
							<div class="text-osvauld-fieldText opacity-60 text-sm">
								{user.isOnline ? "Online" : "Offline"}
							</div>
						</div>
						<div class="flex-shrink-0">
							<CheckVerified />
						</div>
					</button>
				{/each}
			</div>

			<div class="flex justify-end space-x-2 mt-4">
				<button
					onclick={cancel}
					class="px-4 py-2 text-osvauld-fieldText opacity-60 hover:opacity-100 transition-colors duration-200"
				>
					Cancel
				</button>
			</div>
		</div>
	</div>
{/if}
