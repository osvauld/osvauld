<script lang="ts">
	import {
		DownloadIcon,
		UserPlus,
		Devices,
		QrScanner,
		Key,
		Settings,
		ClosePanel,
	} from "@osvauld/icons";
	import { uiState } from "../../state/ui.svelte";
	import AddUserForm from "../ui/AddUserForm.svelte";
	import AddDevice from "../ui/AddDevice.svelte";
	import packageJson from "../../package.json";

	const MENUITEMS = [
		{ id: "add", label: "Add Device", icon: QrScanner },
		// { id: "devices", label: "My Devices", icon: Devices },
		{ id: "addUser", label: "Connect a User", icon: UserPlus },
		{ id: "change", label: "Change Password", icon: Key },
		{ id: "export", label: "Emergency Key", icon: DownloadIcon },
	] as const;

	type MenuItemId = (typeof MENUITEMS)[number]["id"];

	let activeMenuItem = $state<MenuItemId | null>("addUser");

	let menuButtons = $state<Record<string, HTMLButtonElement>>({});

	// Watch for password prompt modal state changes to handle focus
	$effect(() => {
		// When password prompt modal is closed, focus the currently active menu item
		if (!uiState.passwordPromptModal.show) {
			// Use setTimeout to ensure DOM is updated and focus works properly
			setTimeout(() => {
				if (activeMenuItem) {
					const activeButton = menuButtons[activeMenuItem];
					activeButton?.focus();
				}
			}, 0);
		}
	});

	const handleSettingSelection = (id: MenuItemId) => {
		switch (id) {
			case "add":
			case "addUser":
				activeMenuItem = id;
				break;
			// case "devices":
			// 	activeMenuItem = "addUser";
			// 	break;
			case "export":
				uiState.showPasswordPrompt(false);
				break;
			case "change":
				uiState.showPasswordPrompt(true);
				break;
		}
	};
</script>

<div class="grow max-h-full overflow-hidden flex text-4xl text-white">
	<nav
		class="w-[17rem] shrink-0 h-full max-h-full flex flex-col py-4 px-2 border-r border-osvauld-borderColor"
	>
		<div
			class="flex justify-between items-center gap-2 text-osvauld-fieldText text-sm pl-3 select-none cursor-default"
		>
			<span class="flex items-center gap-2"
				><Settings size={20} />Settings
			</span>
			<button
				class="cursor-pointer p-1.5 hover:text-osvauld-sideListTextActive"
				aria-label="Close settings"
				onclick={() => (uiState.profileViewLayout = false)}
				><ClosePanel size={20} /></button
			>
		</div>
		<div
			class="border-b border-osvauld-borderColor text-osvauld-fieldText flex flex-col my-3 py-1 gap-1"
		></div>
		<div
			class="grow flex flex-col gap-2 px-1 py-2 text-white text-sm font-normal whitespace-nowrap"
		>
			{#each MENUITEMS as { id, label, icon: Icon }}
				<button
					bind:this={menuButtons[id]}
					class="group w-full flex items-center gap-3 px-3 py-2 rounded-lg text-osvauld-fieldText transition-colors duration-150 cursor-pointer hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive focus:outline-1 outline-livnotePink"
					class:text-osvauld-sideListTextActive={id === activeMenuItem}
					class:bg-osvauld-fieldActive={id === activeMenuItem}
					aria-label={label}
					onclick={() => handleSettingSelection(id)}
				>
					<span>
						<Icon color="currentColor" size={20} />
					</span>
					<span>{label}</span>
				</button>
			{/each}

			<div class="mt-auto flex items-center justify-center">
				<span class="text-osvauld-fieldText text-sm"
					>Version {packageJson.version}</span
				>
			</div>
		</div>
	</nav>
	<div class="flex-1 p-4">
		<div class="h-full w-full">
			{#if activeMenuItem === "addUser"}
				<AddUserForm />
			{/if}
			{#if activeMenuItem === "add"}
				<AddDevice />
			{/if}
		</div>
	</div>
</div>
