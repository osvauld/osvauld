<script lang="ts">
	import { onMount } from "svelte";
	import { fade, slide } from "svelte/transition";
	import { sendMessage } from "@osvauld/password-manager-common";
	import ClosePanel from "@osvauld/password-manager-common/icons/closePanel.svelte";
	import InfoIcon from "@osvauld/password-manager-common/icons/infoIcon.svelte";
	import Lens from "@osvauld/password-manager-common/icons/lens.svelte";

	export let showShareList = false;
	export let shareUserList;
	export let noteId;
	let inputRef;
	let selectedUsername = null;
	let isFocused = false;
	let query = "";

	const EXISTING_COLLABORATORS = [
		{
			username: "fusernames",
			online: true,
		},
		{
			username: "rusername",
			online: false,
		},
		{
			username: "vusername",
			online: false,
		},
		{
			username: "dusername",
			online: false,
		},
		{
			username: "yusername",
			online: false,
		},
		{
			username: "xusername",
			online: true,
		},
		{
			username: "zusername",
			online: true,
		},
		{
			username: "lusername",
			online: false,
		},
	];

	const AVAILABLE_COLLABORATORS = [
		{
			username: "gusername",
			online: true,
		},
		{
			username: "vusername",
			online: false,
		},
		{
			username: "dusername",
			online: false,
		},
		{
			username: "yusername",
			online: false,
		},
		{
			username: "xusername",
			online: true,
		},
		{
			username: "zusername",
			online: true,
		},
		{
			username: "lusername",
			online: false,
		},
		{
			username: "musername",
			online: true,
		},
		{
			username: "nusername",
			online: true,
		},
	];

	const toggleCheck = (username) => {
		selectedUsername = selectedUsername === username ? null : username;
	};

	const handleUserIdSelection = async (id: string, publicKey: string) => {
		console.log(id, publicKey);
		// await sendMessage("shareResource", { publicKey, resourceId: $noteId });
		showShareList = false;
	};

	const extractIconLetter = (username) => {
		return username.trim().split("")[0];
	};

	const sortOnlineCollaborators = (availableCollaborators) => {
		return availableCollaborators.sort(
			(a, b) => Number(b.online) - Number(a.online),
		);
	};

	const autofocus = () => {
		inputRef.focus();
	};

	const unfocus = () => {
		inputRef.blur();
	};

	const handleKeyDown = (event: KeyboardEvent) => {
		if (event.key === "Enter" || event.key === " ") {
			autofocus();
		} else if (event.key === "Backspace" && !query.trim()) {
			isFocused = false;
			unfocus();
		}
	};

	onMount(() => {
		//if exisitng collaborators are zero, auto focus the input and always during the auto focus, bring up the other list.
		// We can show that all available users added
		if (EXISTING_COLLABORATORS.length === 0) {
			autofocus();
		}
	});
</script>

<div
	class="absolute top-full right-0 mt-2 z-50 w-[35rem] h-[26.125rem] rounded-2xl border border-osvauld-activeBorder text-osvauld-fieldText bg-osvauld-frameblack p-5 flex flex-col"
	in:fade
	out:fade>
	<div class="flex justify-between items-center">
		<span class="text-3xl text-osvauld-quarzowhite">Add Collaborators</span>
		<button
			class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive"
			aria-label="Close panel">
			<ClosePanel />
		</button>
	</div>
	<!-- <div
		class="mt-5 bg-osvauld-fieldActive rounded-lg flex justify-between items-center p-3 text-sm">
		<p>For easy live collaboration, invite users having livnote account.</p>
		<button aria-label="More info">
			<InfoIcon />
		</button>
	</div> -->

	<div
		class="h-[2.75rem] w-full mt-4 mb-1.5 px-3 py-2.5 flex justify-start items-center border border-osvauld-iconblack focus-within:border-osvauld-activeBorder rounded-lg cursor-pointer"
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
			on:focusin="{() => (isFocused = true)}"
			on:focusout="{() => (isFocused = false)}"
			bind:this="{inputRef}"
			bind:value="{query}" />
		<!-- 	on:click="{getSearchData}"
				on:input="{handleInputChange}"
				bind:value="{query}"
				on:keyup="{handleKeyDown}" -->
	</div>

	<div class="relative p-4">
		<div
			class=" h-[16.25rem] max-h-[16.25rem] overflow-y-auto scrollbar-thin select-none cursor-default">
			{#if EXISTING_COLLABORATORS.length === 0}
				<div class="p-3">No exisiting collaborators found!</div>
			{:else}
				{#each sortOnlineCollaborators(EXISTING_COLLABORATORS) as collaborator}
					<div
						class="flex justify-start items-center gap-2 py-2 pl-2 pr-3.5 mb-3">
						<span
							class="capitalize text-xl px-2.5 py-1 rounded-lg bg-osvauld-fieldActive"
							>{extractIconLetter(collaborator.username)}</span>
						<span class="font-normal text-base max-w-[16rem] truncate"
							>{collaborator.username}</span>
						{#if collaborator.online}
							<span
								class="border border-osvauld-sideListHighlight rounded-lg flex justify-start items-center gap-1 px-2 py-0.5 text-sm text-liveGreen"
								>Online</span>
						{/if}
						<span
							class="ml-auto px-3 py-1.5 rounded-lg bg-lavenderLight text-lavenderText text-sm"
							>Editor</span>
					</div>
				{/each}
			{/if}
		</div>
		{#if isFocused}
			<div
				class="absolute top-0 left-0 w-full h-[95%] rounded-2xl p-3 border border-osvauld-activeBorder bg-osvauld-frameblack">
				{#if AVAILABLE_COLLABORATORS.length === 0}
					<div class="p-3">No exisiting collaborators found!</div>
				{:else}
					<div
						class="h-full max-h-full overflow-y-auto scrollbar-thin p-1 pr-4 select-none">
						{#each sortOnlineCollaborators(AVAILABLE_COLLABORATORS) as collaborator}
							<div
								class="group flex justify-start items-center gap-2 py-2 pl-2 pr-3.5 mb-3 cursor-pointer hover:shadow-[0_0_0_1px_#292A36] hover:rounded-lg hover:bg-osvauld-fieldActive transition-colors ease-in duration-150"
								role="button"
								tabindex="0"
								on:click|stopPropagation="{() => {}}">
								<span
									class="capitalize text-xl px-2.5 py-1 rounded-lg bg-osvauld-fieldActive">
									{extractIconLetter(collaborator.username)}
								</span>
								<span class="font-normal text-base max-w-[16rem] truncate">
									{collaborator.username}
								</span>
								{#if collaborator.online}
									<span
										class="border border-osvauld-sideListHighlight rounded-lg flex justify-start items-center gap-1 px-2 py-0.5 text-sm text-liveGreen">
										Online
									</span>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>
		{/if}
	</div>

	<!-- {#if AVAILABLE_COLLABORATORS.length >= 1}
		<div
			class="max-h-[14.25rem] overflow-y-auto scrollbar-thin p-1 pr-4 select-none">
			{#each sortOnlineCollaborators(AVAILABLE_COLLABORATORS) as collaborator}
				<div
					class="group flex justify-start items-center gap-2 py-2 pl-2 pr-3.5 mb-3 cursor-pointer hover:shadow-[0_0_0_1px_#292A36] hover:rounded-lg hover:bg-osvauld-fieldActive transition-colors ease-in duration-150"
					role="button"
					tabindex="0"
					on:click|stopPropagation="{() => toggleCheck(collaborator.username)}">
					<span
						class="capitalize text-xl px-2.5 py-1 rounded-lg bg-osvauld-fieldActive">
						{extractIconLetter(collaborator.username)}
					</span>
					<span class="font-normal text-base max-w-[16rem] truncate">
						{collaborator.username}
					</span>
					{#if collaborator.online}
						<span
							class="border border-osvauld-sideListHighlight rounded-lg flex justify-start items-center gap-1 px-2 py-0.5 text-sm text-liveGreen">
							Online
						</span>
					{/if}
					<input
						type="checkbox"
						checked="{selectedUsername === collaborator.username}"
						class="ml-auto group-hover:border-osvauld-placeholderblack w-4 h-4 checked:bg-livnotelavender active:outline-none focus:ring-offset-0 focus:ring-0 cursor-pointer" />
				</div>
			{/each}
		</div>
		<div class="border-b border-b-mobile-bgHighlight my-4"></div>
	{/if} -->

	<!-- {#each shareUserList as { id, username, publicKey }}
		<button
			class="profileBtn"
			on:mouseenter="{() => (hoveredItem = id)}"
			on:mouseleave="{() => (hoveredItem = '')}"
			on:click|stopPropagation="{() => handleUserIdSelection(id, publicKey)}">
			{username}
		</button>
	{/each} -->

	<!-- <div class="flex justify-between items-center gap-6 font-medium">
		<button class="flex-1 cursor-pointer">Cancel</button>
		<button
			class="flex-1 border border-osvauld-activeBorder rounded-lg py-2.5 cursor-pointer {selectedUsername
				? 'text-osvauld-ninjablack bg-livnotelavender'
				: 'text-mobile-textActive'}">
			Confirm</button>
	</div> -->
</div>
