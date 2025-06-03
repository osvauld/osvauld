<script lang="ts">
	import {
		CopyIcon,
		DownloadIcon,
		UserPlus,
		Sync,
		Devices,
		QrScanner,
		Key,
		Settings,
	} from "@osvauld/password-manager-common";
	import AddUserForm from "../forms/AddUserForm.svelte";

	// Menu items definition with const assertion for better type safety
	const MENUITEMS = [
		{ id: "connect", label: "Connect", icon: Sync },
		{ id: "add", label: "Add Device", icon: QrScanner },
		{ id: "devices", label: "My Devices", icon: Devices },
		{ id: "addUser", label: "Add User", icon: UserPlus },
		{ id: "change", label: "Change Password", icon: Key },
		{ id: "export", label: "Emergency Key", icon: DownloadIcon },
	] as const;

	// Extract the union type from MENUITEMS for type safety
	type MenuItemId = typeof MENUITEMS[number]['id'];

	// Single state to track the currently active menu item
	let activeMenuItem = $state<MenuItemId | null>("addUser");

	const handleSettingSelection = (id: MenuItemId) => {
		activeMenuItem = id;
	}

</script>

<div class="grow max-h-full overflow-hidden flex text-4xl text-white">
	<nav
		class="w-[22.5rem] shrink-0 h-full max-h-full flex flex-col py-10 px-4 border-r border-osvauld-borderColor">
		<h1 class="flex justify-start items-center gap-2 text-osvauld-fieldText  text-xl pl-6 select-none cursor-default"><span><Settings /></span> Settings</h1>
		<div class="border-b border-osvauld-borderColor text-osvauld-fieldText flex flex-col my-4 py-1 gap-1"></div>
		<div class="grow flex flex-col gap-3 pl-3 py-6 text-white text-base whitespace-nowrap">
		{#each MENUITEMS as { id, label, icon: Icon }}
			<button
				class="group w-full flex items-center gap-3 p-3 rounded-lg text-osvauld-fieldText transition-colors cursor-pointer hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive"
				class:text-osvauld-sideListTextActive={id === activeMenuItem}
				class:bg-osvauld-fieldActive={id === activeMenuItem}
				aria-label={label}
				onclick={() => handleSettingSelection(id)}>
				<span >
					<Icon color="currentColor" size={24} />
				</span>
				<span>{label}</span>
			</button>
		{/each}
		</div>
	</nav>
	<div class="flex-1 min-w-[25rem] py-10 px-8 overflow-hidden">
		{#if activeMenuItem === "addUser"}
			<AddUserForm />
		{/if}
		<!-- Add other forms here as you implement them -->
		<!-- 
		{#if activeMenuItem === "devices"}
			<DevicesForm />
		{/if}
		{#if activeMenuItem === "change"}
			<ChangePasswordForm />
		{/if}
		-->
	</div>
</div>