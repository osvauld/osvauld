<script lang="ts">
	import { Lens, ClosePanel } from "@osvauld/password-manager-common";
	import { sendMessage } from "@osvauld/password-manager-common/";

	// Import the centralized state
	import { dataState, uiState } from "../../state/";
	// Define interfaces
	interface Collaborator {
		username: string;
		online: boolean;
		id?: string;
		publicKey?: string;
	}

	// Props
	interface Props {
		showShareList?: boolean;
	}

	let { showShareList = $bindable(false) }: Props = $props();

	// Local state
	const MAX_ALLOWED_USERS = 1;
	let inputRef = $state<HTMLInputElement | null>(null);
	let selectedUsers = $state<string[]>([]);
	let isFocused = $state(false);
	let query = $state("");
	let focusedIndex = $state(-1);
	let items = $state<HTMLButtonElement[]>([]);
	let availableCollaborators = $state<Collaborator[]>([]);
	let existingCollaborators = $state<Collaborator[]>([]);

	// Computed values
	let availableCollaboratorsFiltered = $derived(() => {
		return query
			? availableCollaborators.filter((c) =>
					c.username.toLowerCase().includes(query.toLowerCase()),
				)
			: availableCollaborators;
	});

	async function fetchUsers() {
		try {
			const users = await sendMessage("getKnownUsers");
			availableCollaborators = users;

			// Here you would typically also fetch existing collaborators for the note
			if (dataState.currentNote?.id) {
				const noteCollaborators = await sendMessage("getNoteCollaborators", {
					noteId: dataState.currentNote.id,
				}).catch(() => []);
				existingCollaborators = noteCollaborators || [];
			}
		} catch (error) {
			console.error("Error fetching users:", error);
			uiState.showToast("Failed to load users", false);
		}
	}

	// Event handlers
	const selectCollaborator = (username: string): void => {
		if (selectedUsers.length >= MAX_ALLOWED_USERS) return;
		query = "";
		selectedUsers = [...selectedUsers, username];
		focusedIndex = -1;
	};

	const handleKeyDown = (event: KeyboardEvent) => {
		const collaboratorsLength = availableCollaboratorsFiltered.length;

		switch (event.key) {
			case " ":
			case "Enter":
				event.preventDefault(); // Prevent space from scrolling
				if (focusedIndex === -1) {
					autofocus();
				} else if (
					collaboratorsLength > 0 &&
					focusedIndex >= 0 &&
					focusedIndex < collaboratorsLength &&
					selectedUsers.length < MAX_ALLOWED_USERS
				) {
					// Select the collaborator when Enter/Space is pressed
					selectCollaborator(
						availableCollaboratorsFiltered[focusedIndex].username,
					);
				}
				break;

			case "Escape":
				// Close dropdown
				isFocused = false;
				unfocus();
				break;

			case "Backspace":
				const isQueryEmpty = query.trim().length === 0;
				const hasSelectedUsers = selectedUsers.length > 0;

				if (hasSelectedUsers && !isQueryEmpty) {
					// Regular backspace on input
				} else if (hasSelectedUsers) {
					selectedUsers = selectedUsers.slice(0, -1);
				} else if (isQueryEmpty) {
					isFocused = false;
					unfocus();
					focusedIndex = -1;
				}
				break;

			case "ArrowDown":
				event.preventDefault(); // Prevent scrolling
				if (collaboratorsLength > 0) {
					focusedIndex =
						focusedIndex === -1 ? 0 : (focusedIndex + 1) % collaboratorsLength;
					scrollToFocusedItem();
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
					scrollToFocusedItem();
				}
				break;

			case "Tab":
				// Let Tab work normally for focus navigation but reset the selection
				focusedIndex = -1;
				break;
		}
	};

	const scrollToFocusedItem = () => {
		if (items[focusedIndex]) {
			items[focusedIndex].scrollIntoView({
				block: "center",
				behavior: "smooth",
			});
		}
	};

	const handleCollaboratorSelection = async (): Promise<void> => {
		if (selectedUsers.length === 0) return;

		const user = availableCollaborators.find(
			(u) => u.username === selectedUsers[0],
		);
		if (!user || !user.id) return;

		try {
			await sendMessage("shareResource", {
				resourceId: dataState.currentNote?.id,
				userId: user.id,
			});

			// Show success toast
			uiState.showToast("Note shared successfully", true);

			// Close the share panel
			showShareList = false;
		} catch (error) {
			console.error("Error sharing note:", error);
			uiState.showToast("Failed to share note", false);
		}
	};

	// Helper functions
	const extractIconLetter = (username: string): string => {
		return username.charAt(0).toUpperCase();
	};

	const sortOnlineCollaborators = (
		collaborators: Collaborator[],
	): Collaborator[] => {
		return [...collaborators].sort(
			(a, b) => Number(b.online) - Number(a.online),
		);
	};

	const filterSelectedUsers = (users: Collaborator[]): Collaborator[] => {
		return users.filter((user) => !selectedUsers.includes(user.username));
	};

	const autofocus = (): void => {
		if (inputRef) inputRef.focus();
	};

	const unfocus = (): void => {
		if (inputRef) inputRef.blur();
	};

	// Initialize data when component is shown
	$effect(() => {
		if (showShareList) {
			fetchUsers();

			// Reset state on reopening
			selectedUsers = [];
			query = "";
			focusedIndex = -1;
			isFocused = false;
		}
	});
</script>

<div
	class="absolute top-full right-20 mt-2 z-50 w-[35rem] {isFocused
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
			onclick={(e) => {
				e.preventDefault();
				showShareList = false;
			}}>
			<ClosePanel />
		</button>
	</div>

	<!-- Search input -->
	<button
		type="button"
		class="h-[2.75rem] w-full mt-4 mb-1.5 px-3 py-2 gap-1 flex justify-start items-center border border-osvauld-iconblack focus-within:border-osvauld-activeBorder rounded-lg cursor-pointer"
		onclick={(e) => {
			e.stopPropagation();
			autofocus();
		}}
		aria-label="Search for collaborators">
		<span class="shrink-0 mr-2">
			<Lens color={isFocused ? "#67697C" : "#30363D"} />
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
			placeholder={selectedUsers.length === 0 ? "Search..." : ""}
			autocorrect="off"
			autocomplete="off"
			aria-controls="collaborators-listbox"
			aria-autocomplete="list"
			onfocusin={() => (isFocused = true)}
			onfocusout={(event) => {
				if (
					event.relatedTarget &&
					event.relatedTarget instanceof Element &&
					event.relatedTarget.closest(".collaborator-list")
				) {
					return;
				}
				isFocused = false;
			}}
			onkeydown={handleKeyDown}
			bind:this={inputRef}
			bind:value={query} />
	</button>

	<!-- Collaborators display area -->
	<div class="relative p-4">
		<div
			class="{isFocused
				? 'h-[16.25rem]'
				: 'h-auto'} max-h-[16.25rem] overflow-y-auto scrollbar-thin select-none cursor-default"
			role="region"
			aria-label="Current collaborators">
			{#if existingCollaborators.length === 0}
				<div class="p-3">No existing collaborators found!</div>
			{:else if isFocused}
				<div class="p-3">Select collaborator</div>
			{:else}
				{#each sortOnlineCollaborators(existingCollaborators) as collaborator}
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

		<!-- Available collaborators dropdown -->
		{#if isFocused}
			<div
				class="absolute top-0 left-0 w-full {selectedUsers.length !== 0
					? 'h-[72%]'
					: 'h-[95%]'}  rounded-2xl p-3 border border-osvauld-activeBorder bg-osvauld-frameblack"
				role="dialog"
				aria-label="Available collaborators">
				{#if availableCollaboratorsFiltered.length === 0 || filterSelectedUsers(availableCollaboratorsFiltered).length === 0}
					<div class="p-3">Users not found!</div>
				{:else}
					<div
						id="collaborators-listbox"
						class="max-h-full overflow-y-auto scrollbar-thin p-1 pr-4 select-none"
						role="listbox"
						aria-label="Available collaborators">
						{#each sortOnlineCollaborators(filterSelectedUsers(availableCollaboratorsFiltered)) as collaborator, index}
							<button
								type="button"
								class="w-full text-left group flex justify-start items-center gap-2 py-2 pl-2 pr-3.5 mb-3 cursor-pointer {focusedIndex ===
								index
									? 'bg-osvauld-fieldActive shadow-[0_0_0_1px_#292A36] rounded-lg'
									: 'hover:shadow-[0_0_0_1px_#292A36] hover:rounded-lg hover:bg-osvauld-fieldActive'} transition-colors ease-in duration-150 collaborator-list"
								role="option"
								id="collaborator-option-{index}"
								aria-selected={focusedIndex === index}
								tabindex={focusedIndex === index ? 0 : -1}
								bind:this={items[index]}
								onmousedown={(e) => {
									e.preventDefault();
									e.stopPropagation();
									selectCollaborator(collaborator.username);
								}}>
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
							</button>
						{/each}
					</div>
				{/if}
			</div>
			{#if selectedUsers.length !== 0}
				<button
					class="absolute bottom-5 left-0 mt-2 w-full py-2.5 rounded-lg font-normal bg-livnotelavender flex justify-center items-center text-osvauld-ninjablack cursor-pointer"
					onmousedown={handleCollaboratorSelection}>
					Add to collaborate
				</button>
			{/if}
		{/if}
	</div>
</div>
