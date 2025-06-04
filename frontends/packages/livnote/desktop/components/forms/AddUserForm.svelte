<script lang="ts">

	import { sendMessage } from "@osvauld/password-manager-common";
	import { uiState } from "../../state/ui.svelte";


	let userDetails = $state("");
	let isSubmitting = $state(false);


	function handleClear() {
		userDetails = ""; // Clear the textarea
	}

	const handleAddUser = async (event: any) => {
		try {
			const userResponse = await sendMessage("addKnownUser", event);
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
	};


	async function handleSubmit(e: Event) {
		e.preventDefault();
		if (!userDetails.trim()) return;
		
		isSubmitting = true;
		try {
			await handleAddUser(userDetails.trim());
			userDetails = ""; // Clear form on success
		} finally {
			isSubmitting = false;
		}
	}

</script>

<div class="h-full flex flex-col text-base">
	<!-- Header Section -->
	<div class="border-b border-osvauld-borderColor pb-6 mb-8">
		<h1 class="text-2xl font-semibold text-white mb-2">Add User</h1>
		<p class="text-osvauld-fieldText text-sm">
			Add a new user to your workspace by entering their user id below.
		</p>
	</div>

	<!-- Form Section -->
	<div class="flex-1">
		<form onsubmit={handleSubmit} class="space-y-6 flex flex-col h-full">
			<div class="space-y-2 grow flex flex-col">
				<label
					for="userDetails"
					class="block text-sm font-medium text-white">
					User ID
				</label>
				<textarea
					id="userDetails"
					bind:value={userDetails}
					placeholder="Paste user id here..."
					rows="10"
					required
					class="w-full px-4 py-3 bg-osvauld-frameblack border border-osvauld-addfieldgrey rounded-lg text-white placeholder-osvauld-fieldText focus:outline-none focus:ring-2 focus:ring-osvauld-carolinablue focus:border-transparent resize-none transition-colors grow"
				></textarea>
				<p class="text-xs text-osvauld-fieldText">
					The user id is a unique public key and used to identify them over internet. It should be in the correct format as provided.
				</p>
			</div>

			<!-- Action Buttons -->
			<div class="flex justify-end gap-3 pt-4 mt-2">
				<button
					type="button"
					onclick={handleClear}
					class="px-6 py-3 border border-osvauld-addfieldgrey text-osvauld-fieldText hover:text-white hover:border-white rounded-lg focus:outline-none focus:ring-2 focus:ring-white focus:ring-offset-2 focus:ring-offset-osvauld-frameblack transition-colors">
					Clear
				</button>
				<button
				type="submit"
				disabled={!userDetails.trim() || isSubmitting}
				class=" bg-osvauld-carolinablue  disabled:bg-gray-600 disabled:cursor-not-allowed text-osvauld-frameblack font-bold cursor-pointer py-3 px-16 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-osvauld-frameblack transition-colors">
				{isSubmitting ? 'Adding User...' : 'Add User'}
			</button>
			</div>
		</form>
	</div>
</div>
