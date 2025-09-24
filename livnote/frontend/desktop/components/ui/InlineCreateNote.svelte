<script lang="ts">
	import { dataState, uiState } from "../../state";
	import { createEmptyNoteContent } from "../notes/documentUtils";
	import { sendMessage } from "../../utils/helper";

	interface Props {
		folderId: string;
		onCancel: () => void;
		onComplete: () => void;
	}

	let { folderId, onCancel, onComplete }: Props = $props();

	let noteTitle = $state("");
	let isCreating = $state(false);
	let inputRef: HTMLInputElement;

	const isDisabled = $derived(noteTitle.trim().length === 0 || isCreating);

	// Auto-focus the input when component mounts
	$effect(() => {
		if (inputRef) {
			inputRef.focus();
		}
	});

	async function handleSubmit(event: Event) {
		event.preventDefault();

		if (isDisabled) return;

		isCreating = true;

		try {
			// Create note content with the specified title
			const noteContent = createEmptyNoteContent(
				dataState.clientId,
				dataState.userDetails?.username,
				noteTitle.trim(),
			);

			const note = await sendMessage("addCredential", {
				resourcePayload: JSON.stringify(noteContent),
				folderId: folderId === "all" ? dataState.currentVault.id : folderId,
				resourceType: "notes",
			});

			// Switch to the new note
			dataState.setCurrentNoteData(note);
			dataState.setCurrentNoteId(note.id);
			uiState.toggleNoteViewLayout(true);

			onComplete();
		} catch (error) {
			console.error("Failed to create note:", error);
			// Could show error toast here
		} finally {
			isCreating = false;
		}
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (event.key === "Escape") {
			event.preventDefault();
			onCancel();
		}
	}

	function handleCancel() {
		onCancel();
	}
</script>

<form
	class="bg-osvauld-modalFieldActive border border-osvauld-defaultBorder rounded-lg p-3 my-1"
	onsubmit={handleSubmit}
	aria-label="Create new note"
>
	<div class="flex flex-col gap-3">
		<!-- Input field -->
		<div>
			<label for="note-title" class="sr-only">Note title</label>
			<input
				id="note-title"
				bind:this={inputRef}
				bind:value={noteTitle}
				onkeydown={handleKeyDown}
				type="text"
				class="w-full bg-osvauld-frameblack text-osvauld-sideListTextActive placeholder-osvauld-fieldText border border-osvauld-borderColor rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-livnotelavender focus:border-livnotelavender transition-colors duration-150"
				placeholder="Enter note title"
				autocomplete="off"
				autocorrect="off"
				disabled={isCreating}
				aria-describedby="note-title-hint"
			/>
			<div id="note-title-hint" class="sr-only">
				Enter a title for the new note. Press Escape to cancel.
			</div>
		</div>

		<!-- Action buttons -->
		<div class="flex gap-2 justify-end">
			<button
				type="button"
				onclick={handleCancel}
				class="px-3 py-1.5 text-sm rounded-md text-osvauld-fieldText hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive transition-colors duration-150 focus:outline-none focus:ring-2 focus:ring-livnotelavender focus:ring-offset-2 focus:ring-offset-osvauld-modalFieldActive"
				disabled={isCreating}
			>
				Cancel
			</button>
			<button
				type="submit"
				class="px-3 py-1.5 text-sm rounded-md transition-colors duration-150 focus:outline-none focus:ring-2 focus:ring-livnotelavender focus:ring-offset-2 focus:ring-offset-osvauld-modalFieldActive disabled:opacity-50 disabled:cursor-not-allowed
					{isDisabled
					? 'bg-osvauld-iconblack text-osvauld-fieldText'
					: 'bg-livnotelavender text-primarydark hover:bg-opacity-90'}"
				disabled={isDisabled}
				aria-describedby="create-note-button-hint"
			>
				{isCreating ? "Creating..." : "Create"}
			</button>
			<div id="create-note-button-hint" class="sr-only">
				{isDisabled
					? "Enter a note title to enable creation"
					: "Create the new note"}
			</div>
		</div>
	</div>
</form>
