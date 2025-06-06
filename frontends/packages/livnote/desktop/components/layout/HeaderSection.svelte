<script lang="ts">
	import { slide } from "svelte/transition";

	import {
		sendMessage,
		writeToClipboard,
	} from "@osvauld/password-manager-common";

	import {
		CopyIcon,
		RightArrow,
		Logout,
		Profile,
		Settings,
	} from "@osvauld/password-manager-common";

	// Import the centralized state
	import { dataState, uiState } from "../../state";

	// Local UI state
	let showDropdown = $state(false);
	let hoveredItem = $state("");

	// Menu items definition with const assertion for better type safety
	const MENUITEMS = [
		{ id: "settings", label: "Settings", icon: Settings },
		{ id: "userid", label: "Copy UserID", icon: CopyIcon },
		{ id: "logout", label: "Logout", icon: Logout },
	] as const;

	// Extract the union type from MENUITEMS for type safety
	type MenuItemId = typeof MENUITEMS[number]['id'];

	// Handle dropdown menu item clicks
	const handleDropDownClick = async (id: MenuItemId) => {
		switch (id) {
			case "logout":
				await sendMessage("logout");
				uiState.setWelcomeScreen(true);
				break;
			case "userid":
				try {
					const userDetails = await sendMessage("getUserDetailsForShare");
					await writeToClipboard(userDetails);
					uiState.showToast("UserID copied to clipboard", true);
				} catch (error) {
					console.error("Error copying user ID:", error);
					uiState.showToast("Failed to copy UserID", false);
				}
				break;
			case "settings":
				uiState.toggleProfileViewLayout();
				break;
		}
		showDropdown = false;
	};

	// Close dropdown when clicking outside
	const handleOutsideClick = (e: MouseEvent) => {
		showDropdown = false;
	};
</script>

<div class="h-32 w-full border-b border-osvauld-borderColor flex">
	<span
		role="button"
		tabindex="0"
		aria-label="Go to home view"
		class="basis-[360px] shrink-0 h-full flex items-center justify-center text-5xl font-semibold text-[#8A86E5] leading-none tracking-tight " 
		onclick={() => uiState.toggleProfileViewLayout(false)}
		onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); uiState.toggleProfileViewLayout(false); } }}>
		Livnote
	</span>
	<div class="grow py-10 px-16 flex items-center justify-start">
		<!-- <div
			class="flex h-12 w-full min-w-[400px] max-w-2xl items-center bg-osvauld-frameblack py-2.5 px-3 rounded-lg mr-3">
			<span class="sr-only">Search</span>
			<Lens color="#4D4F60" />
			<input
				type="text"
				name="search"
				class="ml-4 grow border-0 focus:ring-0 outline-0 bg-osvauld-frameblack text-osvauld-activeBorder placeholder:text-osvauld-activeBorder font-light text-base leading-6"
				placeholder="Search..." />
		</div> -->
		<div
			class="relative ml-auto text-osvauld-fieldText font-normal text-sm z-40">
			<button
				aria-label="Open Profile View"
				class="w-[16.5rem] p-3 rounded-lg bg-osvauld-frameblack flex justify-start items-center"
				onclick={() => (showDropdown = !showDropdown)}>
				<Profile color="#4D4F60" />
				<span class="ml-2 capitalize">{dataState.userDetails?.username}</span>
				<span
					class="ml-auto transition-transform ease-linear"
					class:rotate-90={showDropdown}>
					<RightArrow />
				</span>
			</button>
			{#if showDropdown}
				<div
					class="bg-transparent fixed inset-0 z-40"
					role="presentation"
					aria-hidden="true"
					onclick={handleOutsideClick}>
				</div>
				<ul
					class="absolute top-[120%] left-0 z-50 w-[16.5rem] rounded-xl border border-osvauld-borderColor bg-osvauld-ninjablack p-3 flex flex-col gap-3"
					in:slide
					out:slide>
					{#each MENUITEMS as { id, label, icon: Icon }}
					<button
					onmouseenter={() => (hoveredItem = id)}
					onmouseleave={() => (hoveredItem = "")}
					onclick={(e) => {
						e.stopPropagation();
						handleDropDownClick(id);
					}}>
					<li class="profileBtn">
							<Icon
								color={hoveredItem === id ? "#F2F2F0" : "#85889C"}
								size={24} />
							{label}
						</li>
						</button>
					{/each}
					</ul>
			{/if}
		</div>
	</div>
</div>
