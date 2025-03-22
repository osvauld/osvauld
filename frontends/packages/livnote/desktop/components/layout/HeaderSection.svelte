<script lang="ts">
	import { slide, fly } from "svelte/transition";
	import OsvauldDesktopLogo from "@osvauld/password-manager-common/icons/osvauldDesktopLogo.svelte";
	import Lens from "@osvauld/password-manager-common/icons/lens.svelte";
	import Profile from "@osvauld/password-manager-common/icons/profile.svelte";
	import Key from "@osvauld/password-manager-common/icons/key.svelte";
	import UserPlus from "@osvauld/password-manager-common/icons/userPlus.svelte";
	import CopyIcon from "@osvauld/password-manager-common/icons/copyIcon.svelte";
	import downloadIcon from "@osvauld/password-manager-common/icons/downloadIcon.svelte";
	import {
		sendMessage,
		writeToClipboard,
	} from "@osvauld/password-manager-common/utils/helper";

	import RightArrow from "@osvauld/password-manager-common/icons/rightArrow.svelte";
	import { addDeviceModal, showConnector } from "../../store/desktop.ui.store";
	import Sync from "@osvauld/password-manager-common/icons/sync.svelte";
	import Devices from "@osvauld/password-manager-common/icons/devices.svelte";
	import QrScanner from "@osvauld/password-manager-common/icons/qrScanner.svelte";
	import Logout from "@osvauld/password-manager-common/icons/logout.svelte";
	import {
		showWelcome,
		language,
		showSyncQr,
		toastStore,
		showAddUser,
		passwordPromptModal,
	} from "../../store/desktop.ui.store";

	let showDropdown = false;
	let hoveredItem = "";

	const MENUITEMS = [
		{ id: "connect", label: "Connect", icon: Sync },
		{ id: "userid", label: "Copy UserID", icon: CopyIcon },
		{ id: "add", label: "Add Device", icon: QrScanner },
		{ id: "devices", label: "My Devices", icon: Devices },
		{ id: "addUser", label: "Add User", icon: UserPlus },
		{ id: "change", label: "Change Password", icon: Key },
		{ id: "export", label: "Emergency Key", icon: downloadIcon },
		{ id: "logout", label: "Logout", icon: Logout },
	];

	const handleDropDownClick = async (id: string) => {
		switch (id) {
			case "add":
				addDeviceModal.set(true);
				break;
			case "logout":
				await sendMessage("logout");
				showWelcome.set(true);
				break;
			case "sync":
				showSyncQr.set(true);
				break;
			case "connect":
				showConnector.set(true);
				break;
			case "userid":
				const userId = await sendMessage("getPublicKey");
				await writeToClipboard(userId);
				toastStore.set({
					show: true,
					message: "UserID copied to clipboard",
					success: true,
				});
				break;
			case "addUser":
				showAddUser.set(true);
				break;
			case "export":
				passwordPromptModal.set({ isChangePassword: false, show: true });
				break;
			case "change":
				passwordPromptModal.set({ isChangePassword: true, show: true });
				break;
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
