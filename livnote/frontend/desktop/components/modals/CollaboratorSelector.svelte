<script lang="ts">
	import { Lens, ClosePanel } from "../../icons";

	// Define interfaces
	interface Collaborator {
		username: string;
		online: boolean;
		id?: string;
		publicKey?: string;
	}

	// Props
	interface Props {
		show?: boolean;
		title?: string;
		availableUsers?: Collaborator[];
		existingUsers?: Collaborator[];
		maxSelections?: number;
		buttonText?: string;
		variant?: "note" | "folder";
		onShare?: (selectedUsers: { username: string; id: string }[]) => void;
		onClose?: () => void;
	}

	let {
		show = $bindable(false),
		title = "Invite to collaborate",
		availableUsers = [],
		existingUsers = [],
		maxSelections = 1,
		buttonText = "Add to collaborate",
		variant = "note",
		onShare = () => {},
		onClose = () => {},
	}: Props = $props();

	// Local state
	let inputRef = $state<HTMLInputElement | null>(null);
	let selectedUsers = $state<string[]>([]);
	let isFocused = $state(false);
	let query = $state("");
	let focusedIndex = $state(-1);
	let items = $state<HTMLButtonElement[]>([]);

	// Computed values
	let availableUsersFiltered = $derived.by(() => {
		// Filter out existing users and apply search query
		const filteredByExisting = availableUsers.filter(
			(user) =>
				!existingUsers.some((existing) => existing.username === user.username),
		);

		return query
			? filteredByExisting.filter((c) =>
					c.username.toLowerCase().includes(query.toLowerCase()),
				)
			: filteredByExisting;
	});

	// Event handlers
	const selectCollaborator = (username: string): void => {
		if (selectedUsers.length >= maxSelections) return;
		query = "";
		selectedUsers = [...selectedUsers, username];
		focusedIndex = -1;
	};

	const handleKeyDown = (event: KeyboardEvent) => {
		const collaboratorsLength = availableUsersFiltered.length;

		switch (event.key) {
			case " ":
			case "Enter":
				event.preventDefault();
				if (focusedIndex === -1) {
					autofocus();
				} else if (
					collaboratorsLength > 0 &&
					focusedIndex >= 0 &&
					focusedIndex < collaboratorsLength &&
					selectedUsers.length < maxSelections
				) {
					selectCollaborator(availableUsersFiltered[focusedIndex].username);
				}
				break;

			case "Escape":
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
				event.preventDefault();
				if (collaboratorsLength > 0) {
					focusedIndex =
						focusedIndex === -1 ? 0 : (focusedIndex + 1) % collaboratorsLength;
					scrollToFocusedItem();
				}
				break;

			case "ArrowUp":
				event.preventDefault();
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

	const handleCollaboratorSelection = (): void => {
		if (selectedUsers.length === 0) return;

		// Get full user objects for selected users
		const selectedUserObjects = selectedUsers
			.map((username) => availableUsers.find((u) => u.username === username))
			.filter((user): user is Collaborator => user !== undefined && !!user.id)
			.map((user) => ({ username: user.username, id: user.id! }));

		if (selectedUserObjects.length > 0) {
			onShare(selectedUserObjects);
			// Reset state after sharing
			selectedUsers = [];
			query = "";
			isFocused = false;
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

	const autofocus = (): void => {
		if (inputRef) inputRef.focus();
	};

	const unfocus = (): void => {
		if (inputRef) inputRef.blur();
	};

	// Reset state when component is shown
	$effect(() => {
		if (show) {
			selectedUsers = [];
			query = "";
			focusedIndex = -1;
			isFocused = false;
		}
	});
</script>

{#if show}
	<div
		class="absolute {variant === 'folder'
			? 'top-14 right-48'
			: 'top-30 right-10'} mt-2 w-[25rem] h-auto max-h-[27.125rem] rounded-2xl border border-osvauld-activeBorder text-osvauld-fieldText bg-osvauld-frameblack p-5 flex flex-col z-[1000]"
		role="dialog"
		aria-labelledby="dialog-title"
	>
		<div class="flex justify-between items-center">
			<span id="dialog-title" class="text-xl text-osvauld-quarzowhite"
				>{title}</span
			>
			<button
				class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
				aria-label="Close panel"
				onclick={(e) => {
					e.preventDefault();
					onClose();
				}}
			>
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
			aria-label="Search for collaborators"
		>
			<span class="shrink-0 mr-2">
				<Lens color={isFocused ? "#67697C" : "#30363D"} />
			</span>
			<label for="search-collaborators" class="sr-only"
				>Search collaborators</label
			>
			{#if selectedUsers.length !== 0}
				{#each selectedUsers as user}
					<span
						class="border border-osvauld-sideListHighlight bg-osvauld-fieldActive rounded-sm px-3 py-1 text-sm"
						>{user}</span
					>
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
				bind:value={query}
			/>
		</button>

		<!-- Collaborators display area -->
		<div class="relative p-0">
			{#if !isFocused}
				<div class="text-osvauld-quarzowhite/50 text-sm font-light py-1.5">
					Existing collaborators
				</div>
			{/if}
			<div
				class="{isFocused
					? 'min-h-0 h-auto'
					: 'h-auto'} max-h-[12rem] pr-1 overflow-y-auto scrollbar-thin select-none cursor-default"
				role="region"
				aria-label="Current collaborators"
			>
				{#if existingUsers.length === 0 && !isFocused}
					<div class="py-3 text-sm font-light">
						None found. Search to invite.
					</div>
				{:else if !isFocused}
					{#each sortOnlineCollaborators(existingUsers) as collaborator}
						<div class="flex justify-start items-center gap-2 py-2 pr-0.5">
							<span
								class="capitalize text-xl px-2.5 py-1 rounded-lg bg-osvauld-fieldActive"
								aria-hidden="true"
								>{extractIconLetter(collaborator.username)}</span
							>
							<span class="font-normal text-base max-w-[16rem] truncate"
								>{collaborator.username}</span
							>
							{#if collaborator.online}
								<span
									class="border border-liveGreen rounded-sm flex justify-start items-center gap-1 px-2 py-0.5 text-xs text-liveGreen"
								>
									Online
								</span>
							{:else}
								<span class="sr-only">Offline</span>
							{/if}
							<span
								class="ml-auto px-3 py-1.5 rounded-lg bg-lavenderLight text-lavenderText text-sm"
							>
								Editor
							</span>
						</div>
					{/each}
				{/if}
			</div>

			<!-- Available collaborators dropdown -->
			{#if isFocused}
				<div
					class="mt-2 min-h-[6rem] w-full rounded-lg p-1 border border-osvauld-activeBorder bg-osvauld-frameblack"
					role="dialog"
					aria-label="Available collaborators"
				>
					{#if availableUsersFiltered.length === 0}
						<div class="p-3 text-sm font-light">No users found!</div>
					{:else}
						<div
							id="collaborators-listbox"
							class="max-h-[12rem] pr-1 overflow-y-auto scrollbar-thin select-none"
							role="listbox"
							aria-label="Available collaborators"
						>
							{#each availableUsersFiltered as collaborator, index}
								<button
									type="button"
									class="w-full text-left group flex justify-start items-center gap-2 py-2 pl-2 pr-3.5 cursor-pointer {focusedIndex ===
									index
										? 'bg-osvauld-fieldActive rounded-lg'
										: 'hover:rounded-lg hover:bg-osvauld-fieldActive'} transition-colors ease-in duration-150 collaborator-list"
									role="option"
									id="collaborator-option-{index}"
									aria-selected={focusedIndex === index}
									tabindex={focusedIndex === index ? 0 : -1}
									bind:this={items[index]}
									onmousedown={(e) => {
										e.preventDefault();
										e.stopPropagation();
										selectCollaborator(collaborator.username);
									}}
								>
									<span
										class="capitalize text-xl px-2.5 py-1 rounded-lg bg-osvauld-fieldActive transition-colors ease-in duration-150 group-hover:bg-osvauld-activeBorder/40"
										aria-hidden="true"
									>
										{extractIconLetter(collaborator.username)}
									</span>
									<span class="font-normal text-base max-w-[16rem] truncate">
										{collaborator.username}
									</span>
									{#if collaborator.online}
										<span
											class="border border-liveGreen rounded-sm flex justify-start items-center gap-1 px-2 py-0.5 text-xs text-liveGreen"
										>
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
			{/if}

			{#if selectedUsers.length !== 0 && isFocused}
				<button
					class="mt-3 w-full py-2.5 rounded-lg font-normal bg-livnotelavender flex justify-center items-center text-osvauld-ninjablack cursor-pointer"
					onmousedown={handleCollaboratorSelection}
				>
					{buttonText}
				</button>
			{/if}
		</div>
	</div>
{/if}
