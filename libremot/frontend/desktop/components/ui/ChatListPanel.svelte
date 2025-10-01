<script lang="ts">
	import { dataState, uiState } from "../../state/";
	import { sendMessage } from "../../utils/helper";
	import { Lens as SearchIcon, Add as AddIcon } from "@osvauld/icons";

	let searchQuery = $state("");
	let isSearching = $state(false);

	const handleSearch = async () => {
		if (searchQuery.trim()) {
			isSearching = true;
			try {
				const results = await sendMessage("searchResources", {
					query: searchQuery,
				});
				dataState.setSearchResults(results);
			} catch (error) {
				console.error("Search error:", error);
				uiState.showToast("Search failed", false);
			} finally {
				isSearching = false;
			}
		} else {
			dataState.clearSearch();
		}
	};

	const handleAddChat = () => {
		// Show user selection modal to choose who to chat with
		dataState.showUserSelectionModal();
	};

	const handleKeyPress = (event: KeyboardEvent) => {
		if (event.key === "Enter") {
			handleSearch();
		}
	};
</script>

<div
	class="flex flex-col p-3 border-b border-osvauld-borderColor bg-osvauld-frameblack"
>
	<div class="flex items-center justify-between mb-3">
		<h1 class="text-osvauld-fieldText font-semibold text-xl">Chats</h1>
		<button
			onclick={handleAddChat}
			class="bg-livnotelavender text-white p-2 rounded-lg hover:bg-opacity-80 transition-colors duration-200"
			title="New Chat"
		>
			<AddIcon />
		</button>
	</div>
	<div class="relative">
		<input
			type="text"
			placeholder="Search chats..."
			bind:value={searchQuery}
			onkeypress={handleKeyPress}
			class="w-full bg-osvauld-ninjablack border border-osvauld-borderColor rounded-lg px-3 py-2 pr-10 text-sm text-osvauld-fieldText placeholder-osvauld-fieldText opacity-60 focus:outline-none focus:border-livnotelavender"
		/>
		<button
			onclick={handleSearch}
			disabled={isSearching}
			class="absolute right-2 top-1/2 transform -translate-y-1/2 p-1 text-osvauld-fieldText opacity-60 hover:opacity-100"
		>
			<SearchIcon />
		</button>
	</div>
</div>
