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
	let inputRef;
	let selectedUsername = null;

	const EXISTING_COLLABORATORS = [
		{
			username: "fusernames",
			online: true,
		},
		{
			username: "rusername",
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

	const handleKeyDown = (event: KeyboardEvent) => {
		if (event.key === "Enter" || event.key === " ") {
			autofocus();
		}
	};

	onMount(async () => {
		shareUserList = await sendMessage("getKnownUsers");
	});
</script>

<style>
	input[type="checkbox"] {
		appearance: none;
		position: relative;
		width: 18px;
		height: 18px;
		border: 1px solid #21262d;
		border-radius: 2px;
		cursor: pointer;
	}

	input[type="checkbox"]::after {
		content: "";
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: calc(100% - 2px);
		height: calc(100% - 2px);
		background-color: #8a86e5;
		border-radius: 1px;
		z-index: 1;
		opacity: 0;
	}

	input[type="checkbox"]::before {
		content: "";
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: 13px;
		height: 9px;
		background-color: #0d1117;
		clip-path: path(
			"M4.23852 8.3792C3.97185 8.3792 3.70518 8.2792 3.50102 8.075L0.600183 5.2L1.48352 4.31667L4.23852 7.04167L11.5502 0L12.4335 0.883333L4.97602 8.0708C4.77185 8.275 4.50518 8.375 4.23852 8.375V8.3792Z"
		);
		opacity: 0;
		z-index: 10;
		transition: opacity 0.2s ease-in-out;
	}

	input[type="checkbox"]:checked::before {
		opacity: 1;
	}

	input[type="checkbox"]:checked::after {
		opacity: 1;
	}

	/* Add focus state for accessibility */
	input[type="checkbox"]:focus {
		outline: 2px solid #8a86e5;
		outline-offset: 1px;
	}

	.group:hover input[type="checkbox"] {
		border-color: #30363d;
	}
</style>

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

	{#if EXISTING_COLLABORATORS.length >= 1}
		<div
			class="max-h-[6.75rem] overflow-y-auto scrollbar-thin select-none cursor-default">
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
		</div>
		<div class="border-b border-b-mobile-bgHighlight my-4"></div>
	{/if}

	{#if AVAILABLE_COLLABORATORS.length >= 1}
		<div
			class="max-h-[14.25rem] overflow-y-auto scrollbar-thin p-1 pr-4 select-none cursor-pointer">
			{#each sortOnlineCollaborators(AVAILABLE_COLLABORATORS) as collaborator}
				<div
					class="group flex justify-start items-center gap-2 py-2 pl-2 pr-3.5 mb-3 hover:shadow-[0_0_0_1px_#292A36] hover:rounded-lg hover:bg-osvauld-fieldActive transition-colors ease-in duration-150"
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
	{/if}

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
