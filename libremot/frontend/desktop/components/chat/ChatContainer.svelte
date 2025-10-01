<script lang="ts">
	import { dataState, uiState } from "../../state/";
	import ChatListView from "./ChatListView.svelte";
	import ChatWorkspace from "./ChatWorkspace.svelte";
	import ChatEmptyState from "./ChatEmptyState.svelte";
	import HeaderSection from "../layout/HeaderSection.svelte";
</script>

<div class="flex h-full w-full overflow-hidden">
	<!-- Left Panel: Chat List (Full Height) -->
	<div class="w-[280px] border-r border-osvauld-borderColor flex-shrink-0 flex flex-col bg-osvauld-ninjablack">
		<ChatListView />
	</div>

	<!-- Right Panel: Header + Chat Workspace -->
	<div class="flex-1 flex flex-col bg-osvauld-ninjablack min-w-0">
		<!-- Header only above right panel -->
		<HeaderSection />
		
		<!-- Chat content -->
		<div class="flex-1 overflow-hidden">
			{#if dataState.currentChatId}
				<ChatWorkspace />
			{:else}
				<ChatEmptyState />
			{/if}
		</div>
	</div>
</div>
