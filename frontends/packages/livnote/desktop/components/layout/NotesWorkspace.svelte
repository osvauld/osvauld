<script lang="ts">
	import { run, stopPropagation } from "svelte/legacy";

	import { onMount } from "svelte";
	import { setContext } from "svelte";
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
	import {
		MobileHome,
		Add,
		VerticalMenu as Menu,
		BinIcon as Bin,
		Star as EmptyStar,
		FavStar as Star,
		CopyIcon,
		DownloadIcon,
		UserPlus,
		Tick,
		BackArrow,
		RightArrow as Arrow,
	} from "@osvauld/password-manager-common";
	import { sendMessage } from "@osvauld/password-manager-common";

	import NotesListView from "../notes/NotesListView.svelte";
	import VaultManager from "../ui/VaultManager.svelte";
	import ShareNote from "../modals/ShareNote.svelte";
	import Loader from "@osvauld/password-manager-common/components/Loader.svelte";

	import { notesInstance } from "../notes/notes";
	import { extractTitle, getLastModifiedDate } from "../utils/helper";
	import { pdfGenerator } from "../utils/pdfGenerator";
	import { LL } from "@osvauld/password-manager-common/i18n/i18n-svelte";

	interface NoteData {
		title?: string;
		content?: string;
		last_modified?: number;
		last_accessed?: number;
	}

	interface Note {
		id: string;
		data: NoteData;
		favourite?: boolean;
	}

	interface User {
		id: string;
		publicKey: string;
	}

	interface ToastMessage {
		show: boolean;
		message: string;
		success: boolean;
	}

	// Type assertions for store values
	let currentNoteValue = $derived($currentNote as Note);
	let notesValue = $derived($notes as Note[]);
	let notesStore = $derived(
		notes as unknown as { set: (value: Note[]) => void },
	);

	let userId: string;
	let addCredentialHovered = $state(false);
	let deleteBtnHoved = $state(false);
	let vaultManagerActive = $state(false);
	let selectedSection = $state("home");
	let showShareList = $state(false);
	let shareUserList: User[] = $state([]);
	let favSelected = $state(false);
	let noteCopied = $state(false);
	let newNoteTitle = $state("");
	let isEditingTitle = $state(false);
	let inputRef: HTMLInputElement | null = $state(null);
	let showDownloadTooltip = $state(false);
	let isPdfGenerating = $state(false);
	let isFavourite = $state();
	run(() => {
		isFavourite = currentNoteValue?.favourite ?? false;
	});

	let saveNoteAndSwitch: () => void = () => {};
	let saveNoteWithNewTitle: () => void = () => {};

	setContext(
		"saveNoteAndSwitchFunction",
		(fn: () => void) => (saveNoteAndSwitch = fn),
	);
	setContext(
		"saveNoteWithNewTitleFunction",
		(fn: () => void) => (saveNoteWithNewTitle = fn),
	);

	function startEditingTitle() {
		newNoteTitle = currentNoteValue?.data?.title ?? "Untitled note";
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
			currentNote.set({
				...currentNoteValue,
				data: {
					...currentNoteValue?.data,
					title: newNoteTitle,
				},
			});

			saveNoteWithNewTitle();
		}
		isEditingTitle = false;
	}

	function handleKeydown(event: KeyboardEvent) {
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
			} as ToastMessage);
		}
	};

	const handleDropDownClick = async (id: string, publicKey: string) => {
		console.log(id, publicKey);
		await sendMessage("shareResource", { publicKey, resourceId: $noteId });
		showShareList = false;
	};

	const handleFilterSelection = (section: string) => {
		selectedSection = section;
		favSelected = section === "favourites";
	};

	const handleBackButton = () => {
		saveNoteAndSwitch();
	};

	const handleDeleteBtn = (item: "folder" | "note") => {
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
			} as ToastMessage);
			return;
		}

		try {
			const note = await notesInstance.createNote({
				folderId: $currentVault.id,
			});

			noteId.set(note);
			noteViewLayout.set(true);
		} catch (error) {
			console.error("Error creating note:", error);
			toastStore.set({
				show: true,
				message: "Failed to create note",
				success: false,
			} as ToastMessage);
		}
	};

	const handleCopyNote = async () => {
		if (!currentNoteValue?.data) {
			toastStore.set({
				show: true,
				message: "No note content to copy",
				success: false,
			} as ToastMessage);
			return;
		}

		try {
			const copyEvent = new CustomEvent("request-editor-content");
			document.dispatchEvent(copyEvent);
			noteCopied = true;
			setTimeout(() => {
				noteCopied = false;
			}, 1000);
		} catch (error) {
			console.error("Error copying note:", error);
			toastStore.set({
				show: true,
				message: "Failed to copy note content",
				success: false,
			} as ToastMessage);
		}
	};

	const toggleFav = async () => {
		isFavourite = !isFavourite;
		try {
			await sendMessage("toggleFav", {
				resourceId: currentNoteValue?.id,
			});
			const notesWithFavToggleChange = notesValue.map((cred) => {
				if (cred.id === currentNoteValue?.id) {
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
			notesStore.set(notesWithFavToggleChange);
		} catch (err) {
			console.error("Error toggling favorite:", err);
		}
	};

	const handleDownloadPdf = async () => {
		if (!currentNoteValue?.data) {
			toastStore.set({
				show: true,
				message: "No note content to download",
				success: false,
			} as ToastMessage);
			return;
		}

		isPdfGenerating = true;
		const pdfStatus = await pdfGenerator(
			currentNoteValue.data.content ?? "",
			currentNoteValue.data.title ?? "Untitled",
		);
		console.log("pdf status =>", pdfStatus);
		toastStore.set(pdfStatus as ToastMessage);
		isPdfGenerating = false;
	};

	onMount(async () => {
		userId = await sendMessage("getUserId");
	});
</script>

<div class="flex grow max-h-full max-w-[calc(100vw - 22.5rem)]">
	<div class="flex-1 flex flex-col overflow-hidden">
		<div class="py-10 px-11 flex items-center justify-start shrink-0">
			{#if $noteViewLayout}
				<div class="mx-2 flex justify-between items-center max-w-[44rem]">
					<button
						class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive shrink-0 cursor-pointer"
						onclick={handleBackButton}>
						<BackArrow />
					</button>
					{#if isEditingTitle}
						<div
							class="grow mx-5 flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack">
							<input
								bind:this={inputRef}
								bind:value={newNoteTitle}
								maxlength="20"
								onkeydown={handleKeydown}
								onblur={saveTitle}
								class="text-white text-4xl bg-osvauld-frameblack border-0 tracking-wider font-semibold border-transparent focus:border-osvauld-iconblack focus:outline-0 focus:ring-0 active:outline-none focus:ring-offset-0" />
						</div>
					{:else}
						<span
							role="button"
							tabindex="0"
							class="grow truncate mx-5 py-2 font-semibold text-4xl text-osvauld-sideListTextActive"
							on:dblclick={startEditingTitle}
							on:keydown={(e) => e.key === "Enter" && startEditingTitle()}>
							{$currentNote?.data?.last_modified ? $currentNote?.data?.title : "Untitled"}
						</span>
					{/if}

					<button
						class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive shrink-0 cursor-pointer"
						onclick={stopPropagation(toggleFav)}>
						{#if isFavourite}
							<Star />
						{:else}
							<EmptyStar color="#85889C" />
						{/if}
					</button>
				</div>
			{:else}
				<div class="relative shrink-0">
					<button
						class="min-w-[20.25rem] text-[26px] text-osvauld-fieldText font-medium leading-6 bg-osvauld-frameblack rounded-lg border border-osvauld-defaultBorder px-4 py-2 flex justify-between items-center capitalize truncate cursor-pointer"
						aria-label="Switch Vault"
						aria-controls="vaultSelector"
						aria-expanded="false"
						onclick={() => (vaultManagerActive = !vaultManagerActive)}>
						<span class="flex-1 truncate text-left py-1"
							>{$currentVault.id === "all"
								? "All Vaults"
								: $currentVault.name}</span
						><span
							class="shrink-0 transition-transform duration-300 {vaultManagerActive
								? '-rotate-90'
								: 'rotate-90'}"><Arrow color="#F2F2F0" size={24} /></span
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
						onclick={() => handleFilterSelection("home")}
						aria-current={selectedSection === "home" ? "page" : undefined}>
						<MobileHome
							size={20}
							color={selectedSection === "home" ? "#BFC0CC" : "#85889C"} />
						<span>Home</span>
					</button>

					<button
						class="w-full flex items-center gap-2 px-3 py-3 rounded-lg
				   {selectedSection === 'favourites'
							? 'text-osvauld-fieldTextActive bg-osvauld-fieldActive'
							: ''}"
						onclick={() => handleFilterSelection("favourites")}
						aria-current={selectedSection === "favourites"
							? "page"
							: undefined}>
						<EmptyStar
							color={selectedSection === "favourites" ? "#BFC0CC" : "#85889C"}
							size={20} />
						<span>Favourites</span>
					</button>
				</div>
				<div
					class="relative ml-auto shrink-0 gap-4 flex justify-end items-center text-base">
					{#if $currentVault.id !== "all"}
						<button
							class="cursor-pointer rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive"
							onclick={stopPropagation(() => {
								handleDeleteBtn("folder");
							})}
							onmouseenter={() => (deleteBtnHoved = true)}
							onmouseleave={() => (deleteBtnHoved = false)}
							aria-label="Delete Folder"
							><Bin
								color={deleteBtnHoved ? "#FF6A6A" : "#85889C"}
								size={24} /></button>
					{/if}
					<button
						class=" rounded-lg p-2.5 flex justify-center items-center cursor-pointer {addCredentialHovered
							? 'bg-livnotelavender text-primarydark'
							: 'bg-osvauld-fieldActive text-osvauld-fieldText'}"
						onmouseenter={() => (addCredentialHovered = true)}
						onmouseleave={() => (addCredentialHovered = false)}
						onclick={handleAddNote}>
						<span class="mr-2 pl-2">New Note</span>
						<Add
							color={addCredentialHovered ? "#010109" : "#85889C"}
							size={24} />
					</button>
				</div>
			{/if}
		</div>

		<NotesListView {favSelected} />
	</div>
	{#if $noteViewLayout}
		<div class="w-[22.5rem] py-11 px-6 flex flex-col gap-11 items-start shrink-0">
			<div class="shrink-0 gap-4 flex justify-between items-center text-base">
				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
					onclick={handleCopyNote}>
					{#if noteCopied}
						<Tick color="#a6e3a1" />
					{:else}
						<CopyIcon color="#85889C" />
					{/if}
				</button>
				<button
					class=" rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
					onclick={stopPropagation(() => handleDeleteBtn("note"))}>
					<Bin size={24} />
				</button>

				<div class="relative flex justify-center items-center">
					<button
						class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
						onmouseenter={() => (showDownloadTooltip = true)}
						onmouseleave={() => (showDownloadTooltip = false)}
						onclick={handleDownloadPdf}
						aria-label="Download as PDF">
						{#if isPdfGenerating}
							<Loader color="#85889C" />
						{:else}
							<DownloadIcon />
						{/if}
					</button>

					{#if showDownloadTooltip}
						<div
							class="absolute bottom-full mb-2 left-1/2 -translate-x-1/2 bg-osvauld-defaultBorder text-toolTipText text-xs px-2 py-1 rounded shadow-lg whitespace-nowrap">
							Download as PDF
						</div>
					{/if}
				</div>
			</div>

			<div class="flex-1 w-full">
				<div class="relative">
					<button
						onclick={handleShareList}
						class="font-medium flex justify-center items-center py-2.5 px-5 rounded-lg bg-livnotelavender text-primarydark border border-osvauld-iconblack cursor-pointer"
						aria-label="share with users">
						<span class="mr-2 pl-2 whitespace-nowrap">Add collaborators</span>
						<UserPlus color="#010109" size={24} />
					</button>
					{#if showShareList}
						<div
							class="bg-transparent fixed inset-0 z-40"
							role="presentation"
							aria-hidden="true"
							onclick={stopPropagation(() => {
								showShareList = false;
							})}>
						</div>
						<ShareNote bind:showShareList {shareUserList} noteId={$noteId} />
					{/if}
				</div>
			</div>
			<div
				class="border-y-1 border-osvauld-defaultBorder py-6 w-full text-left text-sm">
				<p class="text-statusColor">
					Last edited : {currentNoteValue?.data
						? getLastModifiedDate(
								currentNoteValue.data.last_modified ||
									currentNoteValue.data.last_accessed,
							)
						: "Not available"}
				</p>
			</div>
		</div>
	{/if}
</div>
