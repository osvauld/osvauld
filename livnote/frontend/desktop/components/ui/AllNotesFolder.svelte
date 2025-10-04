<script lang="ts">
	import { MobileHome } from "@osvauld/icons";
	import { dataState } from "../../state";

	// Get actual count of notes for All Notes folder
	const allNotesCount = $derived(() => {
		return dataState.notes.length;
	});

	async function handleAllNotesSelect() {
		const allFolder = dataState.vaults.find((v) => v.id === "all");
		if (allFolder) {
			await dataState.switchVault(allFolder);

			// Clear current note selection when switching to All Notes to show list view
			if (dataState.currentNoteId) {
				dataState.clearCurrentNote();
			}
		}
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (event.key === "Enter" || event.key === " ") {
			event.preventDefault();
			handleAllNotesSelect();
		}
	}

	const isSelected = $derived(dataState.currentVault.id === "all");
</script>

<div class="select-none">
	<!-- All Notes folder header -->
	<div
		class="flex items-center group {isSelected
			? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
			: 'text-osvauld-fieldText hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive'} rounded-lg px-2 py-3"
	>
		<div
			class="flex-1 flex items-center gap-1.5 rounded-lg transition-colors duration-150 min-w-0"
			role="treeitem"
			aria-selected={isSelected}
			tabindex="0"
			onclick={handleAllNotesSelect}
			onkeydown={handleKeyDown}
			aria-label="Select All Notes folder"
		>
			<!-- All Notes icon -->
			<span class="shrink-0">
				<MobileHome color={isSelected ? "#F2F2F0" : "#85889C"} size={20} />
			</span>

			<!-- All Notes name -->
			<span
				class="flex-1 text-left text-sm font-light select-none cursor-default min-w-0 overflow-hidden text-ellipsis whitespace-nowrap"
				title="All Notes"
			>
				All Notes
			</span>

			<!-- Note count badge -->
			{#if allNotesCount() > 0}
				<span
					class="shrink-0 text-xs text-osvauld-fieldText"
					aria-label="{allNotesCount()} notes"
				>
					{allNotesCount()}
				</span>
			{/if}
		</div>
	</div>
</div>
