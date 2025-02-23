<script lang="ts">
	import { slide, fly } from "svelte/transition";
	import OsvauldDesktopLogo from "@osvauld/password-manager-common/icons/osvauldDesktopLogo.svelte";
	import Lens from "@osvauld/password-manager-common/icons/lens.svelte";
	import Profile from "@osvauld/password-manager-common/icons/profile.svelte";
	import Key from "@osvauld/password-manager-common/icons/key.svelte";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";

	import RightArrow from "@osvauld/password-manager-common/icons/rightArrow.svelte";
	import { addDeviceModal, showConnector } from "../../store/desktop.ui.store";
	import Sync from "@osvauld/password-manager-common/icons/sync.svelte";
	import Devices from "@osvauld/password-manager-common/icons/devices.svelte";
	import Discord from "@osvauld/password-manager-common/icons/discord.svelte";
	import QrScanner from "@osvauld/password-manager-common/icons/qrScanner.svelte";
	import Logout from "@osvauld/password-manager-common/icons/logout.svelte";
	import {
		showWelcome1,
		language,
		showSyncQr,
	} from "../../store/desktop.ui.store";

	let showDropdown = false;
	let hoveredItem = "";

	const MENUITEMS = [
		{ id: "connect", label: "Connect", icon: Sync },
		{ id: "add", label: "Add Device", icon: QrScanner },
		{ id: "devices", label: "My Devices", icon: Devices },
		{ id: "ask", label: "Ask in Discord", icon: Discord },
		{ id: "change", label: "Change Password", icon: Key },
		{ id: "logout", label: "Logout", icon: Logout },
	];

	const handleDropDownClick = (id: string) => {
		if (id === "add") {
			addDeviceModal.set(true);
		} else if (id == "logout") {
			sendMessage("logout");
			showWelcome1.set(true);
		} else if (id == "sync") {
			showSyncQr.set(true);
		} else if (id === "connect") {
			showConnector.set(true);
		}

		showDropdown = false;
	};
</script>

<div class="h-32 w-full border-b border-osvauld-borderColor flex">
	<span
		class="basis-[360px] shrink-0 h-full flex items-center justify-center text-4xl font-bold text-osvauld-sideListTextActive">
		Livnote
	</span>
	<div class="grow py-10 px-16 flex items-center justify-start">
		<div
			class="flex h-12 w-full min-w-[400px] max-w-2xl items-center bg-osvauld-frameblack py-2.5 px-3 rounded-lg mr-3">
			<span class="sr-only">Search</span>
			<Lens color="#4D4F60" />
			<input
				type="text"
				name="search"
				class="ml-4 grow border-0 focus:ring-0 outline-0 bg-osvauld-frameblack text-osvauld-activeBorder placeholder:text-osvauld-activeBorder font-light text-base leading-6"
				placeholder="Search..." />
		</div>

		<div
			class="relative ml-auto text-osvauld-fieldText font-normal text-sm z-40">
			<button
				aria-label="Open Profile View"
				class="w-[16.5rem] p-3 rounded-lg bg-osvauld-frameblack flex justify-start items-center"
				on:click="{() => (showDropdown = !showDropdown)}">
				<Profile color="#4D4F60" />
				<span class="ml-2">John Doe</span>
				<span
					class="ml-auto transition-transform ease-linear"
					class:rotate-90="{showDropdown}">
					<RightArrow />
				</span>
			</button>
			{#if showDropdown}
				<div
					class="bg-transparent fixed inset-0 z-40"
					role="presentation"
					aria-hidden="true"
					on:click|stopPropagation="{() => (showDropdown = false)}">
				</div>
				<div
					class="absolute top-[120%] left-0 z-50 w-[16.5rem] rounded-xl border border-osvauld-borderColor bg-osvauld-ninjablack p-3 flex flex-col gap-3"
					in:slide
					out:slide>
					{#each MENUITEMS as { id, label, icon: Icon }}
						<button
							class="profileBtn"
							on:mouseenter="{() => (hoveredItem = id)}"
							on:mouseleave="{() => (hoveredItem = '')}"
							on:click|stopPropagation="{() => handleDropDownClick(id)}">
							<Icon
								color="{hoveredItem === id ? '#F2F2F0' : '#85889C'}"
								size="{24}" />
							{label}
						</button>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</div>
