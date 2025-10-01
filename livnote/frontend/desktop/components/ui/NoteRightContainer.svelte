<script lang="ts">
	import { fly } from "svelte/transition";
	import CommentSidebar from "../notes/CommentSidebar.svelte";
	import {
		BinIcon as Bin,
		CopyIcon,
		DownloadIcon,
		TwoPeople,
		Tick,
		MenuToggle,
	} from "../../icons";
	import Loader from "../../common/Loader.svelte";
	// Import the centralized state
	import { dataState, uiState } from "../../state";
	// Import utilities
	import { getLastModifiedDate, sendMessage } from "../../utils/helper";
	import { pdfGenerator } from "../../utils/pdfGenerator";
	// Import ShareNote component
	import ShareNote from "../modals/ShareNote.svelte";

	// Local UI state
	let showShareList = $state(false);
	let noteCopied = $state(false);
	let isPdfGenerating = $state(false);
	let saved = $state(false);
	let lastModifiedDate = $state("");

	// Handle PDF download
	const handleDownloadPdf = async () => {
		// if (!dataState.currentNote?.data) {
		// 	uiState.showToast("No note content to download", false);
		// 	return;
		// }
		//
		// isPdfGenerating = true;
		// const pdfStatus = await pdfGenerator(
		// 	dataState.currentNote.data.content ?? "",
		// 	dataState.currentNote.data.title ?? "Untitled",
		// );
		//
		// uiState.showToast(pdfStatus.message, pdfStatus.success);
		//
		// isPdfGenerating = false;
	};

	const handleToggleNoteRightPanel = () => {
		uiState.toggleNoteRightPanel();
	};

	const closeNoteRightPanel = () => {
		uiState.toggleNoteRightPanel(false);
		uiState.resetNoteRightPanelManualToggle();
	};

	// Handle copying note content
	const handleCopyNote = async () => {
		if (!dataState.currentNoteId) {
			uiState.showToast("No note content to copy", false);
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
			uiState.showToast("Failed to copy note content", false);
		}
	};

	// Delete button handler
	const handleDeleteBtn = (item: "folder" | "note") => {
		uiState.showDeleteConfirmation(item);
	};

	const saveNoteManual = () => {
		const noteId = dataState.currentNoteId;
		if (noteId) {
			dataState.saveNote(noteId);
		}
	};

	$effect(() => {
		saved = uiState.noteSaved;
	});

	$effect(() => {
		const noteId = dataState.currentNoteId;
		const currentNote = dataState.getCurrentNoteData();
		if (currentNote?.data?.last_modified) {
			lastModifiedDate = getLastModifiedDate(currentNote.data.last_modified);
		} else {
			lastModifiedDate = "";
		}
	});
</script>

{#if uiState.showNoteRightPanel}
	<div
		class="w-[22.5rem] h-full min-h-0 max-h-full py-4 px-4 flex flex-col gap-2 items-start shrink-0 border-l border-osvauld-borderColor"
		in:fly={{ x: 200, duration: 400 }}
	>
		<div class="mr-auto">
			<div
				class="shrink-0 w-full gap-2 flex justify-end items-center text-base"
			>
				<button
					onclick={saveNoteManual}
					class="rounded-lg p-2.5 flex justify-center items-center text-osvauld-fieldText bg-osvauld-fieldActive cursor-pointer min-w-[7rem]"
				>
					{#if saved}
						<span
							class="whitespace-nowrap flex items-center justify-center gap-1 text-toastGreen"
						>
							<Tick color="#9DD062" size={24} />
						</span>
					{:else}
						<span>Save</span>
					{/if}
				</button>
				<button
					class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
					aria-label="Copy note content"
					onclick={handleCopyNote}
				>
					{#if noteCopied}
						<Tick color="#a6e3a1" />
					{:else}
						<CopyIcon color="#85889C" />
					{/if}
				</button>
				<button
					class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
					onclick={(e) => handleDeleteBtn("note")}
				>
					<Bin size={24} />
				</button>

				<div class="relative flex justify-center items-center">
					<button
						class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
						onclick={handleDownloadPdf}
						aria-label="Download as PDF"
						title="Download as PDF"
					>
						{#if isPdfGenerating}
							<Loader color="#85889C" />
						{:else}
							<DownloadIcon />
						{/if}
					</button>
				</div>
			</div>

			<button
				onclick={() => (showShareList = true)}
				class="mt-4 w-full font-normal text-base flex justify-center items-center py-2.5 px-5 rounded-lg bg-livnotelavender text-primarydark border border-osvauld-iconblack cursor-pointer"
				aria-label="Invite Collaborators to edit"
				aria-haspopup="dialog"
				aria-expanded={showShareList}
			>
				<span class="mr-2 pl-2 whitespace-nowrap">Invite to edit</span>
				<TwoPeople size={20} />
			</button>

			{#if showShareList}
				<div
					class="bg-transparent fixed inset-0"
					role="presentation"
					aria-hidden="true"
					onclick={(e) => {
						e.stopPropagation();
						showShareList = false;
					}}
				></div>
				<ShareNote bind:showShareList />
			{/if}
		</div>

		<div class="flex-1 flex flex-col w-full min-h-0 relative">
			<div class="flex-1 min-h-0">
				<CommentSidebar />
			</div>
		</div>

		<div
			class=" border-osvauld-defaultBorder py-3 pb-0 w-full text-left text-sm flex justify-between items-center"
		>
			<button
				class="rounded-lg flex justify-center items-center cursor-e-resize"
				title="Close note right panel"
				onclick={closeNoteRightPanel}
			>
				<MenuToggle />
			</button>
			<p class="text-statusColor">Last modified: {lastModifiedDate}</p>
		</div>
	</div>
{/if}
