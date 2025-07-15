<script lang="ts">
	import CommentSidebar from "../notes/CommentSidebar.svelte";
	import {
		BinIcon as Bin,
		CopyIcon,
		DownloadIcon,
		UserPlus,
		Tick,
		Loader,
	} from "@osvauld/password-manager-common";
	// Import the centralized state
	import { dataState, uiState } from "../../state";

	// Import utilities
	import { getLastModifiedDate } from "../utils/helper";
	import { pdfGenerator } from "../utils/pdfGenerator";
	import { notesInstance } from "../notes/notes";

	// Import ShareNote component
	import ShareNote from "../modals/ShareNote.svelte";

	// Local UI state
	let showShareList = $state(false);
	let noteCopied = $state(false);
	let showDownloadTooltip = $state(false);
	let isPdfGenerating = $state(false);
	let saved = $state(false);

	let lastModifiedTimestamp = $state<number | undefined>(undefined);

	// Handle PDF download
	const handleDownloadPdf = async () => {
		if (!dataState.currentNote?.data) {
			uiState.showToast("No note content to download", false);
			return;
		}

		isPdfGenerating = true;
		const pdfStatus = await pdfGenerator(
			dataState.currentNote.data.content ?? "",
			dataState.currentNote.data.title ?? "Untitled",
		);

		uiState.showToast(pdfStatus.message, pdfStatus.success);

		isPdfGenerating = false;
	};

	// Handle copying note content
	const handleCopyNote = async () => {
		if (!dataState.currentNote?.data) {
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

	// Manual save function
	const saveNoteManual = () => {
		if (!dataState.currentNote) return;

		notesInstance
			.saveNote(dataState.currentNote.data?.title || "Untitled")
			.catch(console.error);

		saved = true;

		setTimeout(() => {
			saved = false;
		}, 1000);
	};

	// Update lastModifiedTimestamp whenever currentNote changes
	$effect(() => {
		const currentTimestamp = dataState.currentNote?.data?.last_modified;
		if (currentTimestamp) {
			lastModifiedTimestamp = currentTimestamp;
		}
	});

	$effect(() => {
		saved = uiState.noteSaved;
	});

	// Function to get initial from name
	const getInitial = (name: string): string => {
		return name.charAt(0).toUpperCase();
	};

</script>

<div class="w-[22.5rem] h-full min-h-0 max-h-full py-11  pb-4 px-6 flex flex-col gap-2 items-start shrink-0">
	<div class="shrink-0 gap-4 flex justify-between items-center text-base">
		<button
			onclick={saveNoteManual}
			class="rounded-lg p-2.5 flex justify-center items-center text-osvauld-fieldText bg-osvauld-fieldActive cursor-pointer min-w-[7rem]">
			{#if saved}
				<span class="whitespace-nowrap flex items-center justify-center">
					<span class="text-[#9DD062] mr-2">Saved</span>
					<Tick color="#9DD062" />
				</span>
			{:else}
				<span>Save</span>
			{/if}
		</button>
		<button
			class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
			onclick={handleCopyNote}>
			{#if noteCopied}
				<Tick color="#a6e3a1" />
			{:else}
				<CopyIcon color="#85889C" />
			{/if}
		</button>
		<button
			class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
			onclick={(e) => handleDeleteBtn("note")}>
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

	<div class="flex-1 flex flex-col w-full min-h-0 relative">
			<button
				onclick={() => (showShareList = true)}
				class="font-medium flex justify-center items-center py-2.5 px-5 rounded-lg bg-livnotelavender text-primarydark border border-osvauld-iconblack cursor-pointer "
				aria-label="share with users">
				<span class="mr-2 pl-2 whitespace-nowrap">Add collaborators</span>
				<UserPlus color="#010109" size={24} />
			</button>

			{#if showShareList}
				<div
					class="bg-transparent fixed inset-0 "
					role="presentation"
					aria-hidden="true"
					onclick={(e) => {
						e.stopPropagation();
						showShareList = false;
					}}>
				</div>
				<ShareNote bind:showShareList />
			{/if}


		<div class="flex-1 min-h-0">
			<CommentSidebar/>
		</div>
		</div>

	<div
		class="border-b-1 border-osvauld-defaultBorder py-3 w-full text-left text-sm">
		<p class="text-statusColor">
			Last modified : {dataState.currentNote?.data
				? getLastModifiedDate(lastModifiedTimestamp)
				: "Not available"}
		</p>
	</div>
</div>
