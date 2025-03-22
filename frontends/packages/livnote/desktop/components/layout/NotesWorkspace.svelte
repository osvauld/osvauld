<script lang="ts">
	import { jsPDF } from "jspdf";
	import { open, BaseDirectory } from "@tauri-apps/plugin-fs";
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
	import Loader from "@osvauld/password-manager-common/components/Loader.svelte";

	let userId;
	let addCredentialHovered = false;
	let deleteBtnHoved = false;
	let vaultManagerActive = false;
	let selectedSection = "home";
	let showShareList = false;
	let shareUserList = [];
	let favSelected = false;
	let noteCopied = false;
	let showDownloadTooltip = false;
	let isPdfGenerating = false;
	let saveNoteAndSwitch = () => {};
	$: isFavourite = $currentNote.favourite;

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
	const handleDownloadPdf = async () => {
		if (!$currentNote || !$currentNote?.data) {
			toastStore.set({
				show: true,
				message: "No note content to download",
				success: false,
			});
			return;
		}

		// Add loading indicator state
		isPdfGenerating = true;

		try {
			// Get the document title for the filename
			const title = extractTitle($currentNote?.data?.content) || "note";
			const safeTitle = title.replace(/[^a-z0-9]/gi, "_").toLowerCase();

			// Get the editor content
			const editorEl = document.querySelector(".ProseMirror");
			if (!editorEl) {
				throw new Error("Editor content not found");
			}

			// Create a container for the content with proper styling
			const container = document.createElement("div");
			container.innerHTML = `
         <div style="width: 100%;">
        <div style="font-family: 'Inter', 'Segoe UI', sans-serif; line-height: 1.5; color: black; ">
          ${editorEl.innerHTML}
        </div>
      </div>
    `;

			// Apply styling fixes for the PDF
			const allElements = container.querySelectorAll("*");
			allElements.forEach((el) => {
				// Ensure all text is visible on white background
				el.style.color = "black";

				// Fix styling for various elements
				if (el.tagName === "PRE" || el.tagName === "CODE") {
					el.style.backgroundColor = "#f0f0f0";
					el.style.padding = "2px 4px";
					el.style.borderRadius = "3px";
					el.style.fontFamily = "monospace";
				}

				if (el.tagName === "BLOCKQUOTE") {
					el.style.borderLeft = "3px solid #ccc";
					el.style.paddingLeft = "10px";
					el.style.margin = "10px 0";
					el.style.color = "#555";
				}

				if (el.tagName === "UL" || el.tagName === "OL") {
					el.style.paddingLeft = "20px";
					el.style.marginTop = "5px";
					el.style.marginBottom = "5px";
				}

				// Prevent any images from breaking across pages
				if (el.tagName === "IMG") {
					el.style.pageBreakInside = "avoid";
					el.style.breakInside = "avoid";
					el.style.display = "block";
					el.style.marginBottom = "20px"; // Add space after images
				}

				// Also prevent figures, tables, and other container elements from breaking
				if (
					el.tagName === "FIGURE" ||
					el.tagName === "TABLE" ||
					el.tagName === "BLOCKQUOTE" ||
					el.tagName === "PRE"
				) {
					el.style.pageBreakInside = "avoid";
					el.style.breakInside = "avoid";
				}

				// For headings, ensure they don't appear at the bottom of a page
				if (["H1", "H2", "H3", "H4", "H5", "H6"].includes(el.tagName)) {
					el.style.pageBreakAfter = "avoid";
					el.style.breakAfter = "avoid";
					el.style.pageBreakBefore = "auto";
					el.style.breakBefore = "auto";
					el.style.marginTop = "20px";
				}
			});

			// Initialize jsPDF
			const pdf = new jsPDF("p", "mm", "a4");
			const pageWidth = 210; // A4 width in mm
			const contentWidth = 170; // Your content width

			// Generate PDF from HTML content
			pdf.html(container, {
				callback: async function (pdf) {
					// Add page numbers to all pages
					const totalPages = pdf.internal.getNumberOfPages();
					for (let i = 1; i <= totalPages; i++) {
						pdf.setPage(i);
						pdf.setFontSize(10);
						pdf.setTextColor(100, 100, 100);
						const pageText = `Page ${i} of ${totalPages}`;
						const pageTextWidth =
							(pdf.getStringUnitWidth(pageText) * 10) /
							pdf.internal.scaleFactor;
						const pageTextX = (pageWidth - pageTextWidth) / 2;
						const pageNumberY = 285; // Approximately 12mm from bottom edge
						pdf.text(pageText, pageTextX, pageNumberY);
					}

					try {
						// Get PDF data as array buffer
						const pdfData = pdf.output("arraybuffer");

						// Convert to Uint8Array for file writing
						const pdfBuffer = new Uint8Array(pdfData);

						// Determine file path in Documents directory
						const filePath = `${safeTitle}.pdf`;

						// Open the file for writing
						const file = await open(filePath, {
							write: true,
							create: true,
							truncate: true,
							baseDir: BaseDirectory.Document,
						});

						// Write the PDF data to the file
						await file.write(pdfBuffer);

						// Close the file
						await file.close();

						toastStore.set({
							show: true,
							message: `Note exported as PDF to Documents folder: ${safeTitle}.pdf`,
							success: true,
						});
					} catch (error) {
						console.error("Error saving PDF file:", error);
						toastStore.set({
							show: true,
							message: `Failed to save PDF file: ${error.message}`,
							success: false,
						});
					} finally {
						isPdfGenerating = false;
					}
				},
				x: 0,
				y: 0,
				width: contentWidth, // A4 width minus margins
				windowWidth: 1000, // Adjust based on your content
				margin: [15, 15, 15, 15],
				autoPaging: "text", // Use text-aware paging
			});
		} catch (error) {
			console.error("Error creating PDF:", error);
			toastStore.set({
				show: true,
				message: `Failed to create PDF: ${error.message}`,
				success: false,
			});
			isPdfGenerating = false;
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
					<span
						class="grow truncate mx-5 font-semibold text-4xl text-osvauld-sideListTextActive"
						>{$currentNote?.data
							? extractTitle($currentNote?.data?.content)
							: "New note"}
					</span>

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

				<div class="relative flex justify-center items-center">
					<button
						class="rounded-lg p-2.5 flex justify-center items-center bg-osvauld-fieldActive cursor-pointer"
						on:mouseenter="{() => (showDownloadTooltip = true)}"
						on:mouseleave="{() => (showDownloadTooltip = false)}"
						on:click="{handleDownloadPdf}"
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
