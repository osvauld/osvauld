<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { fade, slide } from "svelte/transition";
	import { sendMessage } from "@osvauld/password-manager-common";
	import ClosePanel from "@osvauld/password-manager-common/icons/closePanel.svelte";
	import InfoIcon from "@osvauld/password-manager-common/icons/infoIcon.svelte";
	import Lens from "@osvauld/password-manager-common/icons/lens.svelte";
	import { toastStore } from "../../store/desktop.ui.store";

	export let showShareList = false;
	export let shareUserList;
	export let noteId;
	let inputRef;
	let MAX_ALLOWED_USERS = 1;
	let selectedUsers = [];
	let isFocused = false;
	let query = "";
	let focusedIndex = -1;
	let items: HTMLDivElement[] = [];

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
	];

	// const toggleCheck = (username) => {
	// 	selectedUsername = selectedUsername === username ? null : username;
	// };

	const handleUserIdSelection = async (id: string, publicKey: string) => {
		console.log(id, publicKey);
		// await sendMessage("shareResource", { publicKey, resourceId: $noteId });
		showShareList = false;
	};

	const handleCollaboratorSelection = () => {
		showShareList = false;
		toastStore.set({
			show: true,
			message: `${selectedUsers[0]} added as collaborator`,
			success: true,
		});
	};

	const extractIconLetter = (username) => {
		return username.trim().split("")[0];
	};

	const sortOnlineCollaborators = (availableCollaborators) => {
		return availableCollaborators.sort(
			(a, b) => Number(b.online) - Number(a.online),
		);
	};

	const filterSelectedUsers = (availableUsers) => {
		return availableUsers.filter(
			(user) => !selectedUsers.includes(user.username),
		);
	};

	const autofocus = () => {
		inputRef.focus();
	};

	const unfocus = () => {
		inputRef.blur();
	};

	const handleKeyDown = (event: KeyboardEvent) => {
		// Only process keyboard navigation when we have collaborators
		// const filteredCollaborators = query
		// 	? AVAILABLE_COLLABORATORS.filter((c) =>
		// 			c.username.toLowerCase().includes(query.toLowerCase()),
		// 		)
		// 	: AVAILABLE_COLLABORATORS;

		const collaboratorsLength = AVAILABLE_COLLABORATORS.length;
		switch (event.key) {
			case "Enter":
			case " ":
				event.preventDefault(); // Prevent space from scrolling
				if (focusedIndex === -1) {
					autofocus();
				} else if (
					collaboratorsLength > 0 &&
					focusedIndex >= 0 &&
					focusedIndex < collaboratorsLength &&
					selectedUsers.length < MAX_ALLOWED_USERS
				) {
					// Actually select the collaborator when Enter/Space is pressed
					selectedUsers = [
						...selectedUsers,
						AVAILABLE_COLLABORATORS[focusedIndex].username,
					];
					focusedIndex = -1;
					console.log("Selected collaborator:", selectedUsers);
					// Here you would add your actual selection logic, e.g.:
					// handleUserIdSelection(selectedCollaborator.id, selectedCollaborator.publicKey);

					// Important: Keep focus on the input after selection
				}
				break;

			case "Escape":
				// Add escape key handling to close dropdown
				isFocused = false;
				unfocus();
				break;

			case "Backspace":
				if (selectedUsers.length !== 0 && query.trim()) {
					//do the regular backspace on input
					break;
				} else if (selectedUsers.length !== 0) {
					selectedUsers = selectedUsers.slice(0, -1);
				} else if (!query.trim()) {
					isFocused = false;
					unfocus();
					focusedIndex = -1; // Reset selection when unfocusing
				}
				break;

			case "ArrowDown":
				event.preventDefault(); // Prevent scrolling
				if (collaboratorsLength > 0) {
					focusedIndex =
						focusedIndex === -1 ? 0 : (focusedIndex + 1) % collaboratorsLength;
					if (items[focusedIndex]) {
						items[focusedIndex].scrollIntoView({
							block: "center",
							behavior: "smooth",
						});
					}
				}
				break;

			case "ArrowUp":
				event.preventDefault(); // Prevent scrolling
				if (collaboratorsLength > 0) {
					if (focusedIndex === -1) {
						focusedIndex = collaboratorsLength - 1;
					} else {
						focusedIndex =
							(focusedIndex - 1 + collaboratorsLength) % collaboratorsLength;
					}
					if (items[focusedIndex]) {
						items[focusedIndex].scrollIntoView({
							block: "center",
							behavior: "smooth",
						});
					}
				}
				break;

			case "Tab":
				// Let Tab work normally for focus navigation but reset the selection
				focusedIndex = -1;
				break;
		}
	};
	onMount(() => {
		if (EXISTING_COLLABORATORS.length === 0) {
			autofocus();
		}
	});
</script>

<!-- Add these ARIA attributes to the main component -->
<div
	class="absolute top-full right-0 mt-2 z-50 w-[35rem] {isFocused
		? 'h-[26.125rem] '
		: 'h-auto'} rounded-2xl border border-osvauld-activeBorder text-osvauld-fieldText bg-osvauld-frameblack p-5 flex flex-col"
	role="dialog"
	aria-labelledby="dialog-title">
	<div class="flex justify-between items-center">
		<span id="dialog-title" class="text-3xl text-osvauld-quarzowhite"
			>Add Collaborators</span>
		<button
			class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
			aria-label="Close panel"
			on:click="{() => (showShareList = false)}">
			<ClosePanel />
		</button>
	</div>

	<!-- Improve the search input accessibility -->
	<div
		class="h-[2.75rem] w-full mt-4 mb-1.5 px-3 py-2 gap-1 flex justify-start items-center border border-osvauld-iconblack focus-within:border-osvauld-activeBorder rounded-lg cursor-pointer"
		on:click|stopPropagation="{autofocus}"
		role="search">
		<span class="shrink-0 mr-2">
			<Lens color="{isFocused ? '#67697C' : '#30363D'}" />
		</span>
		<label for="search-collaborators" class="sr-only"
			>Search collaborators</label>
		{#if selectedUsers.length !== 0}
			{#each selectedUsers as user}
				<span
					class="border border-osvauld-sideListHighlight bg-osvauld-fieldActive rounded-lg px-3 py-1 text-base"
					>{user}</span>
			{/each}
		{/if}

		<input
			id="search-collaborators"
			type="text"
			class="h-full w-full ml-1 bg-osvauld-frameblack border-0 text-osvauld-quarzowhite placeholder-osvauld-placeholderblack text-base outline-0 focus:ring-0"
			placeholder="{selectedUsers.length === 0 ? 'Search...' : ''}"
			autocorrect="off"
			autocomplete="off"
			aria-controls="collaborators-listbox"
			aria-expanded="{isFocused}"
			aria-autocomplete="list"
			on:focusin="{() => (isFocused = true)}"
			on:focusout="{(event) => {
				if (
					event.relatedTarget &&
					event.relatedTarget.closest('.collaborator-list')
				) {
					return;
				}
				isFocused = false;
			}}"
			on:keydown="{handleKeyDown}"
			bind:this="{inputRef}"
			bind:value="{query}" />
	</div>

	<!-- Make the existing collaborators list accessible -->
	<div class="relative p-4">
		<div
			class="{isFocused
				? 'h-[16.25rem]'
				: 'h-auto'} max-h-[16.25rem] overflow-y-auto scrollbar-thin select-none cursor-default"
			role="region"
			aria-label="Current collaborators">
			{#if EXISTING_COLLABORATORS.length === 0}
				<div class="p-3">No existing collaborators found!</div>
			{:else if isFocused}
				<div class="p-3">Select collaborator</div>
			{:else}
				{#each sortOnlineCollaborators(EXISTING_COLLABORATORS) as collaborator}
					<div
						class="flex justify-start items-center gap-2 py-2 pl-2 pr-3.5 mb-3">
						<span
							class="capitalize text-xl px-2.5 py-1 rounded-lg bg-osvauld-fieldActive"
							aria-hidden="true"
							>{extractIconLetter(collaborator.username)}</span>
						<span class="font-normal text-base max-w-[16rem] truncate"
							>{collaborator.username}</span>
						{#if collaborator.online}
							<span
								class="border border-osvauld-sideListHighlight rounded-lg flex justify-start items-center gap-1 px-2 py-0.5 text-sm text-liveGreen">
								Online
							</span>
						{:else}
							<span class="sr-only">Offline</span>
						{/if}
						<span
							class="ml-auto px-3 py-1.5 rounded-lg bg-lavenderLight text-lavenderText text-sm">
							Editor
						</span>
					</div>
				{/each}
			{/if}
		</div>

		<!-- Make the available collaborators dropdown accessible -->
		{#if isFocused}
			<div
				class="absolute top-0 left-0 w-full {selectedUsers.length !== 0
					? 'h-[72%]'
					: 'h-[95%]'}  rounded-2xl p-3 border border-osvauld-activeBorder bg-osvauld-frameblack"
				role="dialog"
				aria-label="Available collaborators">
				{#if AVAILABLE_COLLABORATORS.length === 0 || filterSelectedUsers(AVAILABLE_COLLABORATORS).length === 0}
					<div class="p-3">Additional users not found!</div>
				{:else}
					<div
						id="collaborators-listbox"
						class=" max-h-full overflow-y-auto scrollbar-thin p-1 pr-4 select-none"
						role="listbox"
						aria-label="Available collaborators">
						{#each sortOnlineCollaborators(filterSelectedUsers(AVAILABLE_COLLABORATORS)) as collaborator, index}
							<div
								class="group flex justify-start items-center gap-2 py-2 pl-2 pr-3.5 mb-3 cursor-pointer {focusedIndex ===
								index
									? 'bg-osvauld-fieldActive shadow-[0_0_0_1px_#292A36] rounded-lg'
									: 'hover:shadow-[0_0_0_1px_#292A36] hover:rounded-lg hover:bg-osvauld-fieldActive'} transition-colors ease-in duration-150 collaborator-list"
								role="option"
								id="collaborator-option-{index}"
								aria-selected="{focusedIndex === index}"
								tabindex="{focusedIndex === index ? 0 : -1}"
								bind:this="{items[index]}"
								on:mousedown|stopPropagation="{(e) => {
									e.preventDefault();
									// Keep focus on the input element after mouse selection
									console.log('Selected collab==>', collaborator.username);
									if (selectedUsers.length >= MAX_ALLOWED_USERS) return;
									selectedUsers = [...selectedUsers, collaborator.username];
									focusedIndex = -1;
								}}"
								on:keydown|stopPropagation="{(e) => {
									if (e.key === 'Enter' || e.key === ' ') {
										e.preventDefault();

										// Add your selection logic here
										if (selectedUsers.length >= MAX_ALLOWED_USERS) return;
										console.log('Selected collab==>', collaborator.username);
										selectedUsers = [...selectedUsers, collaborator.username];
										focusedIndex = -1;
									}
								}}">
								<span
									class="capitalize text-xl px-2.5 py-1 rounded-lg bg-osvauld-fieldActive"
									aria-hidden="true">
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
								{:else}
									<span class="sr-only">Offline</span>
								{/if}
							</div>
						{/each}
					</div>
				{/if}
			</div>
			{#if selectedUsers.length !== 0}
				<button
					class="absolute bottom-5 left-0 mt-2 w-full py-2.5 rounded-lg font-normal bg-livnotelavender flex justify-center items-center text-osvauld-ninjablack cursor-pointer"
					on:mousedown="{handleCollaboratorSelection}"
					>Add to collaborate</button>
			{/if}
		{/if}
	</div>
</div>
