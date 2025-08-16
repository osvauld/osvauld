<script lang="ts">
	import { sendMessage } from "../../utils/helper";
	import { uiState, dataState } from "../../state";

	let userDetails = $state("");
	let isSubmitting = $state(false);

	function handleClear() {
		userDetails = ""; // Clear the textarea
	}

	const handleAddUser = async (userKey: string) => {
		try {
			await sendMessage("addKnownUser", userKey);
			uiState.showToast("User Connected successfully", true);
		} catch (error) {
			uiState.showToast("Failed to connect user", false);
			console.error("Error connecting user:", error);
		}
	};

	async function handleSubmit(e: Event) {
		e.preventDefault();
		if (!userDetails.trim() || isSubmitting) return;

		// Check if user is trying to add their own UserID
		try {
			const inputUserDetails = JSON.parse(atob(userDetails.trim()));
			const currentUserDetails = {
				user_public_key: dataState.userDetails?.publicKey,
				device_public_key: dataState.userDetails?.deviceKey,
				username: dataState.userDetails?.username,
			};
			
			// Compare the input with current user's details
			if (JSON.stringify(inputUserDetails) === JSON.stringify(currentUserDetails)) {
				uiState.showToast("Cannot add your own UserID", false);
				handleClear();
				return;
			}
		} catch (error) {
			// If parsing fails, it might not be a valid UserID format, but we'll let the backend handle that
			console.log("UserID format validation will be handled by backend");
		}

		isSubmitting = true;
		try {
			await handleAddUser(userDetails.trim());
			userDetails = ""; // Clear form on success
		} finally {
			isSubmitting = false;
		}
	}

	function handleKeyDown(e: KeyboardEvent) {
		if (e.key === "Enter" && !e.shiftKey) {
			e.preventDefault();
			handleSubmit(e);
		}
	}
</script>

<div class="h-full flex flex-col text-base">
	<!-- Header Section -->
	<div class="border-b border-osvauld-borderColor pb-6 mb-8">
		<h1 class="text-2xl font-light text-white mb-2">Connect a User</h1>
		<p class="text-osvauld-fieldText text-sm">
			Establish a peer-to-peer connection with a user by entering their user address below.
		</p>
	</div>

	<!-- Form Section -->
	<div class="flex-1">
		<form onsubmit={handleSubmit} class="space-y-6 flex flex-col h-full">
			<div class="space-y-2 grow flex flex-col">
				<label for="userDetails" class="block text-sm font-medium text-white">
					User Address
				</label>
				<textarea
					id="userDetails"
					bind:value={userDetails}
					placeholder="Paste here..."
					rows="10"
					required
					class="w-full px-4 py-3 bg-osvauld-frameblack border border-osvauld-addfieldgrey rounded-lg text-white placeholder-osvauld-fieldText focus:outline-none focus:ring-2 focus:ring-livnotePink focus:border-transparent resize-none transition-colors grow"
					onkeydown={handleKeyDown}></textarea>
				<p class="text-xs text-osvauld-fieldText">
					The address uniquely identifies user over the internet. Vaild for 24 hours after generation. It should be in the correct format as provided.
				</p>
			</div>

			<!-- Action Buttons -->
			<div class="flex justify-end gap-3 pt-4 mt-2">
				<button
					type="button"
					onclick={handleClear}
					class="px-6 py-3 border border-osvauld-addfieldgrey text-osvauld-fieldText hover:text-white hover:border-white rounded-lg focus:outline-none focus:ring-2 focus:ring-white focus:ring-offset-2 focus:ring-offset-osvauld-frameblack transition-colors cursor-pointer">
					Clear
				</button>
				<button
					type="submit"
					disabled={!userDetails.trim() || isSubmitting}
					class=" bg-livnotePink text-osvauld-frameblack font-semibold cursor-pointer py-3 px-16 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:ring-offset-osvauld-frameblack transition-colors">
					{isSubmitting ? "Connecting.." : "Connect"}
				</button>
			</div>
		</form>
	</div>
</div>
