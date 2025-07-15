<script lang="ts">
	import { slide, fly } from "svelte/transition";
	import OsvauldDesktopLogo from "@osvauld/password-manager-common/icons/osvauldDesktopLogo.svelte";
	import Lens from "@osvauld/password-manager-common/icons/lens.svelte";
	import Profile from "@osvauld/password-manager-common/icons/profile.svelte";
	import Key from "@osvauld/password-manager-common/icons/key.svelte";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";

	import RightArrow from "@osvauld/password-manager-common/icons/rightArrow.svelte";
	import { addDeviceModal } from "../../store/desktop.ui.store";
	import Sync from "@osvauld/password-manager-common/icons/sync.svelte";
	import Devices from "@osvauld/password-manager-common/icons/devices.svelte";
	import Discord from "@osvauld/password-manager-common/icons/discord.svelte";
	import QrScanner from "@osvauld/password-manager-common/icons/qrScanner.svelte";
	import Logout from "@osvauld/password-manager-common/icons/logout.svelte";
	import { showWelcome, language, showSyncQr } from "../../store/desktop.ui.store";
	import { LL } from "@osvauld/password-manager-common//i18n/i18n-svelte";
	import {
		LANGUAGE_CODES,
		SUPPORTED_LANGUAGES,
	} from "@osvauld/password-manager-common/utils/translationUtils";


	let showDropdown = false;
	let hoveredItem = "";
	let showLanguageDropdown = false;

	const MENUITEMS = [
		{ id: "sync", label: "Sync", icon: Sync },
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
			showWelcome.set(true);
		} else if (id == "sync") {
			showSyncQr.set(true);
		}
		if (id == "logout") {
			sendMessage("logout");
			showWelcome.set(true);
		}
		showDropdown = false;
	};

	const handleLanguageSelection = (lang: string) => {
		language.set(lang);
		showLanguageDropdown = false;
	};
</script>

<div class="h-32 w-full border-b border-osvauld-borderColor flex">
	<span class="basis-[360px] shrink-0 h-full flex items-center justify-center">
		<OsvauldDesktopLogo />
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
				placeholder={$LL.search()} />
		</div>

		<div class="relative ml-auto">
			<button
				class="capitalize p-3 rounded-lg bg-osvauld-frameblack text-osvauld-fieldText text-sm flex items-center"
				on:click={() => (showLanguageDropdown = !showLanguageDropdown)}
				>{$language}
				<span
					class="ml-auto transition-transform ease-linear"
					class:rotate-90={showLanguageDropdown}>
					<RightArrow />
				</span></button>
			{#if showLanguageDropdown}
				<div
					class="bg-transparent fixed inset-0 z-40"
					role="presentation"
					aria-hidden="true"
					on:click|stopPropagation={() => (showLanguageDropdown = false)}>
				</div>
				<div
					class="absolute top-[120%] right-0 z-50 w-[10rem] rounded-xl border border-osvauld-borderColor bg-osvauld-ninjablack p-1 text-sm"
					in:slide
					out:slide>
					<div
						class="w-full max-h-[12rem] overflow-y-scroll scrollbar-thin flex flex-col gap-2 pr-1">
						{#each LANGUAGE_CODES as language}
							<button
								class="p-2 border border-osvauld-activeBorder rounded-md text-osvauld-fieldText cursor-pointer"
								on:click|stopPropagation={() =>
									handleLanguageSelection(language)}>
								{SUPPORTED_LANGUAGES[language]}
							</button>
						{/each}
					</div>
				</div>
			{/if}
		</div>
		<div class="relative ml-3 text-osvauld-fieldText font-normal text-sm z-40">
			<button
				aria-label="Open Profile View"
				class="w-[16.5rem] p-3 rounded-lg bg-osvauld-frameblack flex justify-start items-center"
				on:click={() => (showDropdown = !showDropdown)}>
				<Profile color="#4D4F60" />
				<span class="ml-2">John Doe</span>
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
					on:click|stopPropagation={() => (showDropdown = false)}>
				</div>
				<div
					class="absolute top-[120%] left-0 z-50 w-[16.5rem] rounded-xl border border-osvauld-borderColor bg-osvauld-ninjablack p-3 flex flex-col gap-3"
					in:slide
					out:slide>
					{#each MENUITEMS as { id, label, icon: Icon }}
						<button
							class="profileBtn"
							on:mouseenter={() => (hoveredItem = id)}
							on:mouseleave={() => (hoveredItem = "")}
							on:click|stopPropagation={() => handleDropDownClick(id)}>
							<Icon
								color={hoveredItem === id ? "#F2F2F0" : "#85889C"}
								size={24} />
							{label}
						</button>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</div>
