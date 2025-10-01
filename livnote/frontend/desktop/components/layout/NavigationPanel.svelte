<script lang="ts">
	import { MenuToggle } from "../../icons";
	import { fly } from "svelte/transition";

	// Import the centralized state
	import { dataState, uiState } from "../../state";

	// Import TreeNavigation, AllNotesFolder, and InlineCreateFolder
	import TreeNavigation from "../ui/TreeNavigation.svelte";
	import AllNotesFolder from "../ui/AllNotesFolder.svelte";
	import InlineCreateFolder from "../ui/InlineCreateFolder.svelte";

	// Create folder state
	let showCreateFolder = $state(false);

	// Toggle navigation panel (close)
	function closeNavigationPanel() {
		uiState.toggleNavigationPanel(false);
		uiState.resetNavigationPanelManualToggle();
		document.documentElement.style.setProperty("--min-editor-width", "0px");
	}

	function handleCreateFolderClick() {
		showCreateFolder = true;
	}

	function handleCreateFolderCancel() {
		showCreateFolder = false;
	}

	function handleCreateFolderComplete() {
		showCreateFolder = false;
	}
</script>

{#if uiState.showNavigationPanel}
	<nav
		class="w-[16rem] shrink-0 h-full pt-4 pb-1 px-1 whitespace-nowrap relative border-r border-osvauld-borderColor flex flex-col"
		in:fly={{ x: -200, duration: 400 }}
		aria-label="Main Navigation"
	>
		{#if uiState.noteViewLayout}
			<button
				aria-label="Collapse navigation panel"
				class="absolute bottom-1 right-1 p-1.5 mb-2 rounded-md transition-colors cursor-w-resize"
				title="Collapse panel"
				onclick={closeNavigationPanel}
			>
				<MenuToggle />
			</button>
		{/if}

		<!-- All Notes Folder -->
		<div class="px-1 mb-2 shrink-0">
			<AllNotesFolder />
		</div>

		<!-- Tree Navigation - This should take remaining space and scroll -->
		<div class="flex-1 overflow-y-auto py-1 scrollbar-thin min-h-0">
			<TreeNavigation />
		</div>

		<!-- Create new folder section -->
		<div class="shrink-0 py-2 border-t border-osvauld-borderColor">
			{#if showCreateFolder}
				<InlineCreateFolder
					onCancel={handleCreateFolderCancel}
					onComplete={handleCreateFolderComplete}
				/>
			{:else}
				<button
					class="w-5/6 flex items-center gap-3 px-3 py-2 rounded-lg text-osvauld-fieldText hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive transition-colors duration-150 focus:outline-none focus:ring-2 focus:ring-livnotelavender focus:ring-offset-2 focus:ring-offset-osvauld-ninjablack"
					onclick={handleCreateFolderClick}
					aria-label="Create new folder"
				>
					<span class="shrink-0 w-4 h-4 flex items-center justify-center">
						<svg
							class="w-3 h-3"
							fill="currentColor"
							viewBox="0 0 12 12"
							aria-hidden="true"
						>
							<path
								d="M6 1a1 1 0 011 1v3h3a1 1 0 110 2H7v3a1 1 0 11-2 0V7H2a1 1 0 110-2h3V2a1 1 0 011-1z"
							></path>
						</svg>
					</span>
					<span class="text-sm font-light">Create new folder</span>
				</button>
			{/if}
		</div>
	</nav>
{/if}
