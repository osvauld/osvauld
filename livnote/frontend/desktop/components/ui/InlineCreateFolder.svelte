<script lang="ts">
	import { sendMessage } from "../../utils/helper";
	import { dataState, uiState } from "../../state";

	interface Props {
		onCancel: () => void;
		onComplete: () => void;
	}

	let { onCancel, onComplete }: Props = $props();

	let folderName = $state("");
	let isCreating = $state(false);
	let inputRef: HTMLInputElement;

	const isDisabled = $derived(folderName.trim().length === 0 || isCreating);

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
			await sendMessage("addFolder", {
				name: folderName.trim(),
				description: "",
			});

			// Refresh vaults and switch to the new folder
			await dataState.fetchVaults();
			const newVault = dataState.vaults.find(
				(vault) => vault.name === folderName.trim(),
			);

			if (newVault) {
				dataState.switchVault(newVault);
			}

			onComplete();
		} catch (error) {
			console.error("Failed to create folder:", error);
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
	class="bg-bgSecondary border border-osvauld-defaultBorder rounded-lg p-3 {uiState.noteViewLayout
		? 'mb-10'
		: ''}"
	onsubmit={handleSubmit}
	aria-label="Create new folder"
>
	<div class="flex flex-col gap-3">
		<!-- Input field -->
		<div>
			<label for="folder-name" class="sr-only">Folder name</label>
			<input
				id="folder-name"
				bind:this={inputRef}
				bind:value={folderName}
				onkeydown={handleKeyDown}
				type="text"
				class="w-full bg-osvauld-frameblack text-osvauld-sideListTextActive placeholder-osvauld-fieldText border border-osvauld-borderColor rounded-md px-3 py-2 text-sm focus:outline-none focus:ring-0.5 focus:ring-livnotelavender focus:border-livnotelavender transition-colors duration-150"
				placeholder="Enter folder name"
				autocomplete="off"
				autocorrect="off"
				disabled={isCreating}
				aria-describedby="folder-name-hint"
			/>
			<div id="folder-name-hint" class="sr-only">
				Enter a name for the new folder. Press Escape to cancel.
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
				aria-describedby="create-button-hint"
			>
				{isCreating ? "Creating..." : "Create"}
			</button>
			<div id="create-button-hint" class="sr-only">
				{isDisabled
					? "Enter a folder name to enable creation"
					: "Create the new folder"}
			</div>
		</div>
	</div>
</form>
