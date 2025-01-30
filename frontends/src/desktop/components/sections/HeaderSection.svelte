<script>
	import { slide, fly } from "svelte/transition";
	import OsvauldDesktopLogo from "../../../icons/osvauldDesktopLogo.svelte";
	import Lens from "../../../icons/lens.svelte";
	import Profile from "../../../icons/profile.svelte";
	import Key from "../../../icons/key.svelte";

	import RightArrow from "../../../icons/rightArrow.svelte";
	import { addDeviceModal } from "../store/desktop.ui.store";
	import Sync from "../../../icons/sync.svelte";
	import Devices from "../../../icons/devices.svelte";
	import Discord from "../../../icons/discord.svelte";
	import QrScanner from "../../../icons/qrScanner.svelte";
	import Logout from "../../../icons/logout.svelte";

	let showDropdown = false;
	let hoveredItem = "";

	const MENUITEMS = [
		{ id: "sync", label: "Sync", icon: Sync },
		{ id: "add", label: "Add Device", icon: QrScanner },
		{ id: "devices", label: "My Devices", icon: Devices },
		{ id: "ask", label: "Ask in Discord", icon: Discord },
		{ id: "change", label: "Change Password", icon: Key },
		{ id: "logout", label: "Logout", icon: Logout },
	];

	const handleDropDownClick = (id) => {
		if (id === "add") {
			addDeviceModal.set(true);
		}
		showDropdown = false;
	};
	import { LL } from "../../../i18n/i18n-svelte";
</script>

<div class="h-32 w-full border-b border-osvauld-borderColor flex">
	<span class="basis-[360px] shrink-0 h-full flex items-center justify-center">
		<OsvauldDesktopLogo />
	</span>
	<div class="grow py-10 px-16 flex items-center justify-between">
		<div
			class="flex h-12 w-full min-w-[400px] max-w-2xl items-center bg-osvauld-frameblack py-2.5 px-3 rounded-lg">
			<span class="sr-only">Search</span>
			<Lens color="#4D4F60" />
			<input
				type="text"
				name="search"
				class="grow border-0 focus:ring-0 outline-0 bg-osvauld-frameblack text-osvauld-activeBorder placeholder:text-osvauld-activeBorder font-light text-base leading-6"
				placeholder="{$LL.search()}" />
		</div>
		<div class="relative ml-3 text-osvauld-fieldText font-normal text-sm z-40">
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
