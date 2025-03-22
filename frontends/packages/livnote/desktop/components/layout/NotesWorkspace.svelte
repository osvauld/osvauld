<script lang="ts">
	import {
		currentVault,
		noteViewLayout,
		toastStore,
		vaults,
		noteId,
		currentNote,
		notes,
		deleteConfirmationModal,
	} from "../../store/desktop.ui.store";
	import { extractTitle, getLastModifiedDate } from "../utils/helper";
	import Add from "@osvauld/password-manager-common/icons/add.svelte";
	import Menu from "@osvauld/password-manager-common/icons/verticalMenu.svelte";
	import Bin from "@osvauld/password-manager-common/icons/binIcon.svelte";
	import EmptyStar from "@osvauld/password-manager-common/icons/star.svelte";
	import Star from "@osvauld/password-manager-common/icons/favStar.svelte";
	import CopyIcon from "@osvauld/password-manager-common/icons/copyIcon.svelte";
	import DownloadIcon from "@osvauld/password-manager-common/icons/downloadIcon.svelte";
	import UserPlus from "@osvauld/password-manager-common/icons/userPlus.svelte";
	import Tick from "@osvauld/password-manager-common/icons/tick.svelte";
	import BackArrow from "@osvauld/password-manager-common/icons/backArrow.svelte";
	import Arrow from "@osvauld/password-manager-common/icons/rightArrow.svelte";
	import NotesListView from "../notes/NotesListView.svelte";
	import { LL } from "@osvauld/password-manager-common/i18n/i18n-svelte";
	import VaultManager from "../ui/VaultManager.svelte";

	import { MobileHome } from "@osvauld/password-manager-common";
	import { sendMessage } from "@osvauld/password-manager-common";
	import { notesInstance } from "../notes/notes";
	import { onMount } from "svelte";
	import { setContext } from "svelte";
	import ShareNote from "../modals/ShareNote.svelte";

	let userId;
	let addCredentialHovered = false;
	let deleteBtnHoved = false;
	let vaultManagerActive = false;
	let selectedSection = "home";
	let showShareList = false;
	let shareUserList = [];
	let favSelected = false;
	let noteCopied = false;
	let newNoteTitle = "";
	let isEditingTitle = false;
	let inputRef;

	let saveNoteAndSwitch = () => {};
	$: isFavourite = $currentNote.favourite;

	function startEditingTitle() {
		newNoteTitle = $currentNote?.data
			? extractTitle($currentNote?.data?.content)
			: "Untitled note";
		isEditingTitle = true;

		// Focus the input after the DOM updates
		setTimeout(() => {
			if (inputRef) {
				inputRef.focus();
				inputRef.select();
			}
		}, 0);
	}

	function saveTitle() {
		if (newNoteTitle.trim()) {
			// Replace this with your actual save logic
			currentNote.set({ ...$currentNote, title: newNoteTitle });
			// Need to do a manual save Note trigger here.
		}
		isEditingTitle = false;
	}

	function handleKeydown(event) {
		if (event.key === "Enter") {
			saveTitle();
		} else if (event.key === "Escape") {
			isEditingTitle = false;
		}
	}

	const handleShareList = async () => {
		shareUserList = await sendMessage("getKnownUsers");
		if (shareUserList.length !== 0) {
			showShareList = true;
		} else {
			toastStore.set({
				show: true,
				message: "Please add users to enable collaboration",
				success: false,
			});
		}
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
		currentNote.set({});
	};

	const handleDeleteBtn = (item: "folder" | "note") => {
		console.log("handleDeleteBtn triggerr===>");
		item === "folder"
			? deleteConfirmationModal.set({ item: "folder", show: true })
			: deleteConfirmationModal.set({ item: "note", show: true });
	};

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

	// Add this function to NotesWorkspace.svelte
	const handleCopyNote = async () => {
		if (!$currentNote || !$currentNote?.data) {
			toastStore.set({
				show: true,
				message: "No note content to copy",
				success: false,
			});
			return;
		}

		try {
			// Get editor content as HTML by communicating with RichTextEditor component
			// Using a custom event to get content
			const copyEvent = new CustomEvent("request-editor-content");
			document.dispatchEvent(copyEvent);
			noteCopied = true;
			setTimeout(() => {
				noteCopied = false;
			}, 1000);
			// The response will come via a different event handler we'll add next
		} catch (error) {
			console.error("Error copying note:", error);
			toastStore.set({
				show: true,
				message: "Failed to copy note content",
				success: false,
			});
		}
	};

	const toggleFav = async () => {
		isFavourite = !isFavourite;
		try {
			await sendMessage("toggleFav", {
				resourceId: $currentNote.id,
			});
			const notesWithFavToggleChange = $notes.map((cred) => {
				if (cred.id === $currentNote.id) {
					return {
						...cred,
						data: {
							...cred.data,
						},
						favourite: isFavourite,
					};
				}
				return cred;
			});
			notes.set(notesWithFavToggleChange);
		} catch (err) {
			console.error("Error toggling favorite:", err);
		}
	};

	onMount(async () => {
		userId = await sendMessage("getUserId");
	});
</script>

<div class="flex grow max-h-full">
	<div class="flex-1 flex flex-col overflow-hidden">
		<div class="py-10 px-11 flex items-center justify-start shrink-0">
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
							size="{20}"
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
						<EmptyStar
							color="{selectedSection === 'favourites' ? '#BFC0CC' : '#85889C'}"
							size="{20}" />
						<span>Favourites</span>
					</button>
				</div>
				<div
					class="relative ml-auto shrink-0 gap-4 flex justify-end items-center text-base">
					{#if $currentVault.id !== "all"}
						<button
							class="cursor-pointer rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive"
							on:click|stopPropagation="{() => {
								handleDeleteBtn('folder');
							}}"
							on:mouseenter="{() => (deleteBtnHoved = true)}"
							on:mouseleave="{() => (deleteBtnHoved = false)}"
							aria-label="Delete Folder"
							><Bin
								color="{deleteBtnHoved ? '#FF6A6A' : '#85889C'}"
								size="{24}" /></button>
					{/if}
					<button
						class=" rounded-lg p-2.5 flex justify-center items-center cursor-pointer {addCredentialHovered
							? 'bg-livnotelavender text-primarydark'
							: 'bg-osvauld-fieldActive text-osvauld-fieldText'}"
						on:mouseenter="{() => (addCredentialHovered = true)}"
						on:mouseleave="{() => (addCredentialHovered = false)}"
						on:click="{handleAddNote}">
						<span class="mr-2 pl-2">New Note</span>
						<Add
							color="{addCredentialHovered ? '#010109' : '#85889C'}"
							size="{24}" />
					</button>
				</div>
			{:else}
				<div class="mx-2 flex justify-between items-center max-w-[44rem]">
					<button
						class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive shrink-0 cursor-pointer"
						on:click="{handleBackButton}">
						<BackArrow />
					</button>
					{#if isEditingTitle}
						<div
							class="grow mx-5 flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack">
							<input
								bind:this="{inputRef}"
								bind:value="{newNoteTitle}"
								maxlength="20"
								on:keydown="{handleKeydown}"
								on:blur="{saveTitle}"
								class="text-white text-4xl bg-osvauld-frameblack border-0 tracking-wider font-semibold border-transparent focus:border-osvauld-iconblack focus:outline-0 focus:ring-0 active:outline-none focus:ring-offset-0" />
						</div>
					{:else}
						<span
							role="button"
							tabindex="0"
							class="grow truncate mx-5 font-semibold text-4xl text-osvauld-sideListTextActive"
							on:dblclick="{startEditingTitle}"
							on:keydown="{(e) => e.key === 'Enter' && startEditingTitle()}">
							{$currentNote?.data
								? extractTitle($currentNote?.data?.content)
								: "New note"}
						</span>
					{/if}

					<button
						class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive shrink-0 cursor-pointer"
						on:click|stopPropagation="{toggleFav}">
						{#if isFavourite}
							<Star />
						{:else}
							<EmptyStar color="#85889C" />
						{/if}
					</button>
				</div>
			{/if}
		</div>

		<NotesListView {favSelected} />
	</div>
	{#if $noteViewLayout}
		<div class="w-[22.5rem] py-11 px-6 flex flex-col gap-11 items-start">
			<div class=" shrink-0 gap-4 flex justify-between items-center text-base">
				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
					on:click="{handleCopyNote}">
					{#if noteCopied}
						<Tick color="#a6e3a1" />
					{:else}
						<CopyIcon color="#85889C" />
					{/if}
				</button>
				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
					on:click|stopPropagation="{() => handleDeleteBtn('note')}">
					<Bin size="{24}" />
				</button>

				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive">
					<DownloadIcon />
				</button>
				<!-- <button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive">
					<Menu />
				</button> -->
			</div>

			<div class="flex-1 w-full">
				<div class="relative">
					<button
						on:click="{handleShareList}"
						class="font-medium flex justify-center items-center py-2.5 px-5 rounded-lg bg-livnotelavender text-primarydark border border-osvauld-iconblack cursor-pointer"
						aria-label="share with users">
						<span class="mr-2 pl-2 whitespace-nowrap">Add collaborators</span>
						<UserPlus color="#010109" size="{24}" />
					</button>
					{#if showShareList}
						<div
							class="bg-transparent fixed inset-0 z-40"
							role="presentation"
							aria-hidden="true"
							on:click|stopPropagation="{() => {
								showShareList = false;
							}}">
						</div>
						<ShareNote bind:showShareList {shareUserList} noteId="{$noteId}" />
					{/if}
				</div>
			</div>
			<div
				class="border-y-1 border-osvauld-defaultBorder py-6 w-full text-left text-sm">
				<p class="text-statusColor">
					Last edited : {$currentNote?.data
						? getLastModifiedDate(
								$currentNote.data.last_modified ||
									$currentNote.data.last_accessed,
							)
						: "Not available"}
				</p>
			</div>
		</div>
	{/if}
</div>
