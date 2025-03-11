<script lang="ts">
	import { onMount } from "svelte";
	import { fade, slide } from "svelte/transition";
	import { sendMessage } from "@osvauld/password-manager-common";

	export let showShareList = false;
	export let noteId;
	let shareUserList = [];
	let hoveredItem;

	const handleUserIdSelection = async (id: string, publicKey: string) => {
		console.log(id, publicKey);
		// await sendMessage("shareResource", { publicKey, resourceId: $noteId });
		showShareList = false;
	};

	onMount(async () => {
		shareUserList = await sendMessage("getKnownUsers");
	});
</script>

<div
	class="absolute top-full right-0 mt-2 z-50 w-[16.5rem] rounded-xl border border-osvauld-borderColor bg-osvauld-ninjablack p-3 flex flex-col gap-3"
	in:fade
	out:fade>
	{#each shareUserList as { id, username, publicKey }}
		<button
			class="profileBtn"
			on:mouseenter="{() => (hoveredItem = id)}"
			on:mouseleave="{() => (hoveredItem = '')}"
			on:click|stopPropagation="{() => handleUserIdSelection(id, publicKey)}">
			{username}
		</button>
	{/each}
</div>
