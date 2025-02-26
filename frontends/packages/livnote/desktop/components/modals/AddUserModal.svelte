<script lang="ts">
	import { onMount } from "svelte";
	import { createEventDispatcher } from "svelte";
	const dispatch = createEventDispatcher();

	let nickname = "";
	let publicKey = "";

	onMount(() => {
		console.log("mounted");
	});

	function handleClose() {
		dispatch("close");
	}

	function handleSubmit() {
		console.log("Adding user:", { nickname, publicKey });
		dispatch("userAdd", { nickname, publicKey });
		// Handle form submission here
	}
</script>

<div
	class="p-4 inset-0 items-center justify-center z-50 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[2px] fixed flex flex-col gap-4">
	<div class="bg-osvauld-frameblack p-6 rounded-lg shadow-lg w-full max-w-md">
		<h1 class="text-2xl font-bold mb-6 text-center">Add User</h1>

		<form on:submit|preventDefault={handleSubmit} class="flex flex-col gap-4">
			<div>
				<label
					for="nickname"
					class="block text-sm font-medium text-gray-700 mb-1">
					Nickname
				</label>
				<input
					id="nickname"
					type="text"
					bind:value={nickname}
					class="w-full px-3 py-2 border border-osvauld-addfieldgrey rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="Enter nickname" />
			</div>

			<div>
				<label
					for="publicKey"
					class="block text-sm font-medium text-gray-700 mb-1">
					Public Key
				</label>
				<input
					id="publicKey"
					type="text"
					bind:value={publicKey}
					class="w-full px-3 py-2 border border-osvauld-addfieldgrey rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="Enter public key" />
			</div>

			<button
				type="submit"
				class="mt-2 w-full bg-osvauld-carolinablue text-osvauld-frameblack font-medium py-2 px-4 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500">
				Add User
			</button>
			<button
				on:click={handleClose}
				class="mt-2 w-full bg-osvauld-carolinablue text-osvauld-frameblack font-medium py-2 px-4 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500">
				Close
			</button>
		</form>
	</div>
</div>
