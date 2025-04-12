<script lang="ts">
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

		saved = true;
		notesInstance
			.saveNote(dataState.currentNote.data?.title || "Untitled")
			.catch(console.error);

		setTimeout(() => {
			saved = false;
		}, 1000);
	};
</script>

<div class="w-[22.5rem] py-11 px-6 flex flex-col gap-11 items-start shrink-0">
	<div class="shrink-0 gap-4 flex justify-between items-center text-base">
		<button
		on:click={saveNoteManual}
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
			on:click={handleCopyNote}>
			{#if noteCopied}
				<Tick color="#a6e3a1" />
			{:else}
				<CopyIcon color="#85889C" />
			{/if}
		</button>
		<button
			class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
			on:click={(e) => handleDeleteBtn("note")}>
			<Bin size={24} />
		</button>

		<div class="relative flex justify-center items-center">
			<button
				class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
				on:mouseenter={() => (showDownloadTooltip = true)}
				on:mouseleave={() => (showDownloadTooltip = false)}
				on:click={handleDownloadPdf}
				aria-label="Download as PDF">
				{#if isPdfGenerating}
					<Loader color="#85889C" />
				{:else}
					<DownloadIcon  />
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
				on:click={() => (showShareList = true)}
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
					on:click={(e) => {
						e.stopPropagation();
						showShareList = false;
					}}>
				</div>
				<ShareNote bind:showShareList />
			{/if}
		</div>
	</div>
	<div
		class="border-y-1 border-osvauld-defaultBorder py-6 w-full text-left text-sm">
		<p class="text-statusColor">
			Last edited : {dataState.currentNote?.data
				? getLastModifiedDate(
						dataState.currentNote.data.last_modified ||
							dataState.currentNote.data.last_accessed ||
							0,
					)
				: "Not available"}
		</p>
	</div>
</div>
