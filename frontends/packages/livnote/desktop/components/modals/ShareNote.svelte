<script lang="ts">
	import { onMount } from "svelte";
	import { fade, slide } from "svelte/transition";
	import { sendMessage } from "@osvauld/password-manager-common";
	import ClosePanel from "@osvauld/password-manager-common/icons/closePanel.svelte";
	import InfoIcon from "@osvauld/password-manager-common/icons/infoIcon.svelte";
	import Lens from "@osvauld/password-manager-common/icons/lens.svelte";

	export let showShareList = false;
	export let noteId;
	let shareUserList = [];
	let hoveredItem;
	let inputRef;

	const handleUserIdSelection = async (id: string, publicKey: string) => {
		console.log(id, publicKey);
		// await sendMessage("shareResource", { publicKey, resourceId: $noteId });
		showShareList = false;
	};

	const autofocus = () => {
		inputRef.focus();
	};

	const handleKeyDown = (event: KeyboardEvent) => {
		if (event.key === "Enter" || event.key === " ") {
			autofocus();
		}
	};

	onMount(async () => {
		shareUserList = await sendMessage("getKnownUsers");
	});
</script>

<div
	class="absolute top-full right-0 mt-2 z-50 w-[35rem] h-[42rem] rounded-2xl border border-osvauld-activeBorder text-osvauld-fieldText bg-osvauld-frameblack p-5 flex flex-col"
	in:fade
	out:fade>
	<div class="flex justify-between items-center">
		<span class="text-3xl text-osvauld-quarzowhite">Invite to edit</span>
		<button
			class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive"
			aria-label="Close panel">
			<ClosePanel />
		</button>
	</div>
	<div
		class="mt-5 bg-osvauld-fieldActive rounded-lg flex justify-between items-center p-3 text-sm">
		<p>For easy live collaboration, invite users having livnote account.</p>
		<button aria-label="More info">
			<InfoIcon />
		</button>
	</div>

	<div
		class="h-[2.75rem] w-full my-4 px-3 py-2.5 flex justify-start items-center border border-osvauld-iconblack focus-within:border-osvauld-activeBorder rounded-lg cursor-pointer"
		on:click|stopPropagation="{autofocus}"
		on:keydown="{handleKeyDown}"
		role="button"
		tabindex="0"
		aria-label="Focus search field">
		<Lens />
		<input
			type="text"
			class="h-full w-full ml-1 bg-osvauld-frameblack border-0 text-osvauld-quarzowhite placeholder-osvauld-placeholderblack text-base outline-0 focus:ring-0"
			placeholder="Search.."
			autocorrect="off"
			autocomplete="off"
			bind:this="{inputRef}" />
		<!-- 	on:click="{getSearchData}"
				on:input="{handleInputChange}"
				bind:value="{query}"
				on:keyup="{handleKeyDown}" -->
	</div>

	<!-- {#each shareUserList as { id, username, publicKey }}
		<button
			class="profileBtn"
			on:mouseenter="{() => (hoveredItem = id)}"
			on:mouseleave="{() => (hoveredItem = '')}"
			on:click|stopPropagation="{() => handleUserIdSelection(id, publicKey)}">
			{username}
		</button>
	{/each} -->
</div>
