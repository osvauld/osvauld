<script lang="ts">
	import {
		currentVault,
		noteViewLayout,
		toastStore,
		vaults,
		noteId,
		refreshCredentialList,
		noteTitle,
	} from "../../store/desktop.ui.store";
	import { slide } from "svelte/transition";
	import Add from "@osvauld/password-manager-common/icons/add.svelte";
	import Menu from "@osvauld/password-manager-common/icons/verticalMenu.svelte";
	import Bin from "@osvauld/password-manager-common/icons/binIcon.svelte";
	import DownArrow from "@osvauld/password-manager-common/icons/downArrow.svelte";
	import Star from "@osvauld/password-manager-common/icons/star.svelte";
	import CopyIcon from "@osvauld/password-manager-common/icons/copyIcon.svelte";
	import DownloadIcon from "@osvauld/password-manager-common/icons/downloadIcon.svelte";
	import UserPlus from "@osvauld/password-manager-common/icons/userPlus.svelte";
	import FavStar from "@osvauld/password-manager-common/icons/favStar.svelte";
	import BackArrow from "@osvauld/password-manager-common/icons/backArrow.svelte";
	import Arrow from "@osvauld/password-manager-common/icons/rightArrow.svelte";
	import NotesListView from "../notes/NotesListView.svelte";
	import { LL } from "@osvauld/password-manager-common/i18n/i18n-svelte";
	import VaultManager from "../ui/VaultManager.svelte";

	import { MobileHome } from "@osvauld/password-manager-common";
	import { sendMessage } from "@osvauld/password-manager-common";
	import { addCredentialHandler } from "@osvauld/password-manager-common";
	import { notesInstance } from "../notes/notes";
	import { onMount } from "svelte";
	import { setContext } from "svelte";

	let userId;
	let addCredentialHovered = false;
	let deleteBtnHoved = false;
	let vaultManagerActive = false;
	let selectedSection = "home";
	let showShareList = false;
	let shareUserList = [];
	let hoveredItem = "";
	let favSelected = false;
	let saveNoteAndSwitch = () => {};

	const handleShareList = async () => {
		shareUserList = await sendMessage("getKnownUsers");
		console.log(shareUserList);
		showShareList = true;
	};

	const handleDropDownClick = async (id: string, publicKey: string) => {
		console.log(id, publicKey);
		await sendMessage("shareResource", { publicKey, resourceId: $noteId });
		showShareList = false;
	};

	setContext("saveNoteAndSwitchFunction", (fn) => (saveNoteAndSwitch = fn));

	const handleFilterSelection = (section) => {
		selectedSection = section;
		// filterFavourites();
		favSelected = !favSelected;
	};

	const handleBackButton = () => {
		saveNoteAndSwitch();
		noteId.set("");
	};

	// const handleDeleteBtn = () => {
	// 	deleteConfirmationModal.set({ item: "folder", show: true });
	// };
	const handleAddNote = async () => {
		if ($vaults.length <= 1 || $currentVault.id === "all") {
			toastStore.set({
				show: true,
				message: "Please add/select folder",
				success: false,
			});
			return;
		}

		try {
			// Create note with initialized state in a single operation
			const note = await notesInstance.createNote({
				folderId: $currentVault.id,
				userId,
			});

			// Set the note ID in the store
			noteId.set(note);

			// Update the view to show the editor
			noteViewLayout.set(true);
		} catch (error) {
			console.error("Error creating note:", error);
			toastStore.set({
				show: true,
				message: "Failed to create note",
				success: false,
			});
		}
	};

	onMount(async () => {
		userId = await sendMessage("getUserId");
	});
</script>

<div class="grow flex flex-col overflow-hidden">
	<div class="py-10 px-16 flex items-center justify-start shrink-0">
		{#if !$noteViewLayout}
			<div class="relative shrink-0">
				<button
					class="min-w-[20.25rem] text-[26px] text-osvauld-fieldText font-medium leading-6 bg-osvauld-frameblack rounded-lg border border-osvauld-defaultBorder px-4 py-2 flex justify-between items-center capitalize truncate cursor-pointer"
					aria-label="Switch Vault"
					aria-controls="vaultSelector"
					aria-expanded="false"
					on:click="{() => (vaultManagerActive = !vaultManagerActive)}">
					<span class="flex-1 truncate text-left py-1"
						>{$currentVault.id === "all"
							? "All Vaults"
							: $currentVault.name}</span
					><span
						class="shrink-0 transition-transform duration-300 {vaultManagerActive
							? '-rotate-90'
							: 'rotate-90'}"><Arrow color="#F2F2F0" size="{24}" /></span
					></button>
				{#if vaultManagerActive}
					<VaultManager bind:vaultManagerActive instance="content" />
				{/if}
			</div>
			<div
				class="mx-6 px-6 border-x border-osvauld-borderColor text-osvauld-fieldText flex gap-6 text-base">
				<button
					class="w-full flex items-center gap-2 px-3 py-3 rounded-lg
                       transition-colors
                       {selectedSection === 'home'
						? 'text-osvauld-fieldTextActive bg-osvauld-fieldActive'
						: ''}"
					on:click="{() => handleFilterSelection('home')}"
					aria-current="{selectedSection === 'home' ? 'page' : undefined}">
					<MobileHome
						size="20"
						color="{selectedSection === 'home' ? '#BFC0CC' : '#85889C'}" />
					<span>Home</span>
				</button>

				<button
					class="w-full flex items-center gap-2 px-3 py-3 rounded-lg
                       {selectedSection === 'favourites'
						? 'text-osvauld-fieldTextActive bg-osvauld-fieldActive'
						: ''}"
					on:click="{() => handleFilterSelection('favourites')}"
					aria-current="{selectedSection === 'favourites'
						? 'page'
						: undefined}">
					<Star
						color="{selectedSection === 'favourites' ? '#BFC0CC' : '#85889C'}"
						size="20" />
					<span>Favourites</span>
				</button>
			</div>
			<div
				class="relative ml-auto shrink-0 gap-4 flex justify-end items-center text-base">
				{#if $currentVault.id !== "all"}
					<button
						class="cursor-pointer rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive"
						on:click|stopPropagation="{() => {}}"
						on:mouseenter="{() => (deleteBtnHoved = true)}"
						on:mouseleave="{() => (deleteBtnHoved = false)}"
						aria-label="Delete Folder"
						><Bin
							color="{deleteBtnHoved ? '#FF6A6A' : '#85889C'}"
							size="24" /></button>
				{/if}
				<button
					class="cursor-pointer rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive"
					on:mouseenter="{() => (addCredentialHovered = true)}"
					on:mouseleave="{() => (addCredentialHovered = false)}"
					on:click="{handleAddNote}">
					<Add color="#85889C" size="24" />
				</button>
			</div>
		{:else}
			<div class="mx-2 flex justify-between items-center max-w-[34rem]">
				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive shrink-0 cursor-pointer"
					on:click="{handleBackButton}">
					<BackArrow />
				</button>
				<span
					class="grow truncate mx-5 font-semibold text-4xl text-osvauld-sideListTextActive"
					>{$noteTitle}
				</span>
				<button
					class="  rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive shrink-0">
					<Star />
				</button>
			</div>

			<div
				class="relative ml-auto shrink-0 gap-4 flex justify-between items-center text-base">
				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive">
					<CopyIcon />
				</button>
				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive">
					<Bin size="24" />
				</button>

				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive">
					<DownloadIcon />
				</button>
				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive">
					<Menu />
				</button>

				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
					on:mouseenter="{() => (addCredentialHovered = true)}"
					on:mouseleave="{() => (addCredentialHovered = false)}"
					on:click="{handleAddNote}">
					<Add color="#85889C" size="24" />
				</button>

				{#if $noteId}
					<button
						on:click="{handleShareList}"
						class=" text-osvauld-textPassive font-medium flex justify-center items-center p-2.5 rounded-lg bg-osvauld-fieldActive border border-osvauld-iconblack cursor-pointer"
						aria-label="share with users">
						<span class="mr-2 pl-2">Invite to edit</span>
						<UserPlus color="#85889C" />
					</button>
				{/if}

				{#if showShareList}
					<div
						class="bg-transparent fixed inset-0 z-40"
						role="presentation"
						aria-hidden="true"
						on:click|stopPropagation="{() => {
							showShareList = false;
						}}">
					</div>
					<div
						class="absolute top-full right-0 mt-2 z-50 w-[16.5rem] rounded-xl border border-osvauld-borderColor bg-osvauld-ninjablack p-3 flex flex-col gap-3"
						in:slide
						out:slide>
						{#each shareUserList as { id, username, publicKey }}
							<button
								class="profileBtn"
								on:mouseenter="{() => (hoveredItem = id)}"
								on:mouseleave="{() => (hoveredItem = '')}"
								on:click|stopPropagation="{() =>
									handleDropDownClick(id, publicKey)}">
								{username}
							</button>
						{/each}
					</div>
				{/if}
			</div>
		{/if}
	</div>

	<NotesListView {favSelected} />
</div>
