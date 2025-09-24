<script lang="ts">
	import { MenuToggle } from "../../icons";
	import { fly } from "svelte/transition";

	// Import the centralized state
	import { dataState, uiState } from "../../state";

	// Import TreeNavigation
	import TreeNavigation from "../ui/TreeNavigation.svelte";

	// Toggle navigation panel (close)
	function closeNavigationPanel() {
		uiState.toggleNavigationPanel(false);
		uiState.resetNavigationPanelManualToggle();
		document.documentElement.style.setProperty("--min-editor-width", "0px");
	}
</script>

{#if uiState.showNavigationPanel}
	<nav
		class="w-[22.5rem] shrink-0 h-full max-h-full py-10 px-4 whitespace-nowrap relative border-r border-osvauld-borderColor"
		in:fly={{ x: -200, duration: 400 }}
		aria-label="Main Navigation"
	>
		<button
			aria-label="Collapse navigation panel"
			class="absolute bottom-1.5 right-3 p-1.5 mb-2 rounded-md transition-colors cursor-w-resize"
			title="Collapse panel"
			onclick={closeNavigationPanel}
		>
			<MenuToggle />
		</button>

		<!-- Tree Navigation -->
		<div class="flex-1 min-h-0 max-h-[95%] overflow-y-auto scrollbar-thin">
			<TreeNavigation />
		</div>
	</nav>
{/if}
