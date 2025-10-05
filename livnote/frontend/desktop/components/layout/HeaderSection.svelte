<script lang="ts">
	import { sendMessage, writeToClipboard } from "../../utils/helper";

	import {
		CopyIcon,
		RightArrow,
		Logout,
		Profile,
		Settings,
		ConnectUser,
		Lens,
		ClosePanel,
	} from "@osvauld/icons";

	// Import the centralized state
	import { dataState, uiState } from "../../state";

	// Local UI state
	let showDropdown = $state(false);
	let hoveredItem = $state("");
	let searchQuery = $state("");
	let searchInput: HTMLInputElement;
	// Menu items definition with const assertion for better type safety
	const MENUITEMS = [
		{ id: "settings", label: "Settings", icon: Settings },
		{ id: "userid", label: "Copy User Address", icon: CopyIcon },
		{ id: "logout", label: "Logout", icon: Logout },
	] as const;

	// Extract the union type from MENUITEMS for type safety
	type MenuItemId = (typeof MENUITEMS)[number]["id"];

	// Handle dropdown menu item clicks
	const handleDropDownClick = async (id: MenuItemId) => {
		switch (id) {
			case "logout":
				await sendMessage("logout");
				dataState.clearAllState();
				uiState.setWelcomeScreen(true);
				break;
			case "userid":
				try {
					const { ucan_token, ucan_pub_key } = await sendMessage(
						"getOneTimeUcanToken",
					);
					const userDetails = {
						user_public_key: dataState.userDetails?.publicKey,
						device_public_key: dataState.userDetails?.deviceKey,
						username: dataState.userDetails?.username,
						ucan_token,
						ucan_pub_key,
					};
					await writeToClipboard(btoa(JSON.stringify(userDetails)));
					uiState.showToast(
						"User Address copied to clipboard, valid for 24 hours",
						true,
					);
				} catch (error) {
					console.error("Error copying user Address:", error);
					uiState.showToast("Failed to copy User Address", false);
				}
				break;
			case "settings":
				uiState.toggleProfileViewLayout();
				break;
		}
		showDropdown = false;
	};

	// Close dropdown when clicking outside
	const handleOutsideClick = async (e: MouseEvent) => {
		showDropdown = false;
	};
	const handleSearch = async (event: Event) => {
		const target = event.target as HTMLInputElement;
		searchQuery = target.value; // Update the bound variable

		// If user types something (trimmed value has characters), switch to All Notes
		if (searchQuery.trim().length > 0) {
			const allNotesFolder = dataState.vaults.find((v) => v.id === "all");
			if (allNotesFolder && dataState.currentVault.id !== "all") {
				await dataState.switchVault(allNotesFolder);
			}
		}

		//console.log("Search query:", searchQuery);
		const noteIds = await sendMessage("searchResource", { query: searchQuery });
		console.log(noteIds);
		dataState.setSearchResults(noteIds);
	};

	// Home button handler - returns to list view
	const handleHomeButton = () => {
		dataState.switchNote(null);
		uiState.toggleNoteRightPanel(true);
		uiState.toggleNoteViewLayout(false);
		uiState.toggleProfileViewLayout(false);
	};
</script>

<div class="h-auto w-full border-b border-osvauld-borderColor flex">
	<div class="grow py-4 px-4 flex items-center justify-end gap-6">
		<div
			class="flex min-w-[400px] items-center bg-osvauld-fieldActive py-2 px-3 rounded-lg focus-within:ring-1 focus-within:ring-livnotePink mr-auto text-sm"
		>
			<span class="sr-only">Search</span>
			<Lens color="#4D4F60" />
			<input
				type="text"
				name="search"
				class="mx-2 grow border-0 focus:ring-0 outline-0 bg-fieldActive text-white placeholder:text-osvauld-activeBorder font-light text-sm leading-6"
				autocorrect="off"
				autocapitalize="off"
				autocomplete="off"
				placeholder="Search..."
				bind:this={searchInput}
				bind:value={searchQuery}
				oninput={handleSearch}
				onkeydown={(e) => {
					if (e.key === "Escape") {
						searchQuery = "";
						dataState.setSearchResults([]);
						searchInput?.blur();
					}
				}}
			/>
			{#if searchQuery}
				<button
					class=" cursor-pointer text-osvauld-activeBorder"
					aria-label="Clear search"
					aria-controls="search-results"
					title="Clear search"
					onclick={() => {
						searchQuery = "";
						dataState.setSearchResults([]);
						searchInput?.focus();
					}}
				>
					<ClosePanel size={18} />
				</button>
			{/if}
		</div>
		<button
			class="flex items-center gap-2 rounded-lg px-5 py-2 border border-livnotePink text-livnotePink hover:bg-livnotePink hover:text-primarydark cursor-pointer"
			aria-label="Open Connect user modal"
			aria-haspopup="dialog"
			aria-controls="connect-user-modal"
			aria-expanded={uiState.connectUserModal.show}
			onclick={() => uiState.showConnectUserModal()}
			onkeydown={(e) => {
				if (e.key === "Enter" || e.key === " ") {
					e.preventDefault();
					uiState.showConnectUserModal();
				}
			}}
		>
			<span class="font-medium text-sm whitespace-nowrap">Connect a User</span>
			<ConnectUser size={20} />
		</button>
		<div class="relative text-textActive font-normal text-base z-40">
			<button
				aria-label="Open Profile View"
				class="px-4 p-2 rounded-lg bg-osvauld-fieldActive flex justify-start items-center"
				onclick={() => (showDropdown = !showDropdown)}
			>
				<Profile color="#a3a4b5" size={20} />
				<span class="ml-2 text-sm capitalize"
					>{dataState.userDetails?.username}</span
				>
				<span
					class="ml-auto transition-transform ease-linear"
					class:rotate-90={showDropdown}
				>
					<RightArrow />
				</span>
			</button>
			{#if showDropdown}
				<div
					class="bg-transparent fixed inset-0 z-40"
					role="presentation"
					aria-hidden="true"
					onclick={handleOutsideClick}
				></div>
				<ul
					class="absolute top-[120%] right-0 z-50 w-[16.5rem] rounded-xl border border-osvauld-borderColor bg-osvauld-ninjablack p-2 text-sm flex flex-col gap-1.5"
				>
					{#each MENUITEMS as { id, label, icon: Icon }}
						<button
							onmouseenter={() => (hoveredItem = id)}
							onmouseleave={() => (hoveredItem = "")}
							onclick={(e) => {
								e.stopPropagation();
								handleDropDownClick(id);
							}}
						>
							<li class="profileBtn cursor-pointer">
								<Icon
									color={hoveredItem === id ? "#F2F2F0" : "#85889C"}
									size={20}
								/>
								{label}
							</li>
						</button>
					{/each}
				</ul>
			{/if}
		</div>
	</div>
</div>
