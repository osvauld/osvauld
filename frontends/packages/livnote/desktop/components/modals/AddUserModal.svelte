<script lang="ts">
	let userDetails = $state("");
	let { close, userAdd } = $props();

	function handleClose() {
		close();
	}

	function handleSubmit(e) {
		e.preventDefault();
		userAdd(userDetails);
		// Handle form submission here
	}
</script>

<div
	class="p-4 inset-0 items-center justify-center z-50 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[2px] fixed flex flex-col gap-4">
	<div class="bg-osvauld-frameblack p-6 rounded-lg shadow-lg w-full max-w-md">
		<h1 class="text-2xl font-bold mb-6 text-center">Add User</h1>

		<form onsubmit={handleSubmit} class="flex flex-col gap-4">
			<div>
				<label
					for="UserDetails"
					class="block text-sm font-medium text-gray-700 mb-1">
					User Details
				</label>
				<input
					id="userDetails"
					type="text"
					bind:value={userDetails}
					class="w-full px-3 py-2 border border-osvauld-addfieldgrey rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="Enter public key" />
			</div>

			<button
				type="submit"
				class="mt-2 w-full bg-osvauld-carolinablue text-osvauld-frameblack font-medium py-2 px-4 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500">
				Add User
			</button>
			<button
				onclick={handleClose}
				class="mt-2 w-full bg-osvauld-carolinablue text-osvauld-frameblack font-medium py-2 px-4 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500">
				Close
			</button>
		</form>
	</div>
</div>
