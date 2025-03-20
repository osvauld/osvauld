<script lang="ts">
	import { SearchedCredential } from "../../dtos/credential.dto";
	import { Key, Lens, Highlight, LinkIcon } from "./icons";
	import { createEventDispatcher } from "svelte";

	export let searchResults: SearchedCredential[] = [];
	export let query = "";
	const dispatch = createEventDispatcher();

	const closeModal = () => {
		dispatch("close", true);
	};

	const handleInputChange = () => {
		dispatch("change", query);
	};

	const handleSearchClick = (result: SearchedCredential) => {
		dispatch("select", result);
	};

	const handleKeyDown = (event: any) => {
		if (event.key === "Escape") {
			closeModal();
		} else if (event.key === "ArrowDown") {
			event.preventDefault();
			focusNextResult();
		} else if (event.key === "ArrowUp") {
			event.preventDefault();
			focusPreviousResult();
		} else if (event.key === "Enter") {
			event.preventDefault();
			if (currentFocusIndex !== -1) {
				handleSearchClick(searchResults[currentFocusIndex]);
			}
		}
	};

	let currentFocusIndex = -1;
	const focusNextResult = () => {
		if (currentFocusIndex < searchResults.length - 1) {
			currentFocusIndex++;
			focusResult(currentFocusIndex);
		}
	};

	const focusPreviousResult = () => {
		if (currentFocusIndex > 0) {
			currentFocusIndex--;
			focusResult(currentFocusIndex);
		}
	};

	const focusResult = (index: number) => {
		const resultElements: any = document.querySelectorAll(".search-result");
		if (resultElements[index]) {
			resultElements[index].focus();
			resultElements[index].setAttribute("aria-selected", "true");
			if (currentFocusIndex !== -1 && currentFocusIndex !== index) {
				resultElements[currentFocusIndex].setAttribute(
					"aria-selected",
					"false",
				);
			}
		}
	};

	function autofocus(node: any) {
		node.focus();
	}
</script>

<style>
	.search-result:focus {
		border: 1px solid #89b4fa;
		outline: none;
	}
</style>

<button
	class="fixed z-10 inset-0 backdrop-filter backdrop-blur-[2px]"
	on:keydown="{handleKeyDown}"
>
	<div class="flex items-start justify-center min-h-screen mt-[2.6vh]">
		<button class="fixed inset-0 bg-black opacity-50" on:click="{closeModal}">
		</button>
		<div
			class="bg-osvauld-frameblack border border-osvauld-iconblack rounded-lg overflow-hidden shadow-xl transform transition-all sm:max-w-2xl sm:w-full !w-[40vw] ml-[10vw]"
		>
			<div
				class="h-[2.2rem] w-[31.25rem] px-2 flex m-auto mt-2 justify-center items-center border border-osvauld-iconblack focus-within:border-osvauld-activeBorder rounded-lg cursor-pointer"
			>
				<Lens />
				<input
					type="text"
					class="h-[2rem] w-full bg-osvauld-frameblack border-0 text-osvauld-quarzowhite placeholder-osvauld-placeholderblack border-transparent text-base focus:border-transparent focus:ring-0 cursor-pointer"
					placeholder="Search.."
					use:autofocus
					on:keyup="{handleInputChange}"
					bind:value="{query}"
				/>
			</div>
			<div class="w-full border-t-[1px] border-osvauld-iconblack my-2"></div>
			<div
				class=" bg-osvauld-frameblack"
				role="listbox"
				aria-label="Search results"
			>
				{#if searchResults.length !== 0}
					<p
						class="text-osvauld-placeholderblack text-sm text-start w-1/2 pl-3"
					>
						Showing results for "{query}"
					</p>
				{/if}
				<div
					class="max-h-64 min-h-32 overflow-y-auto flex justify-start items-center flex-col mb-4 px-3"
				>
					{#each searchResults as result}
						<button
							on:click="{() => handleSearchClick(result)}"
							class="search-result p-2 border rounded-lg border-osvauld-iconblack hover:bg-osvauld-iconblack w-full my-1 flex justify-start items-center min-h-[60px]"
							tabindex="-1"
							role="option"
							aria-selected="false"
						>
							<div
								class="h-full flex justify-center items-center scale-150 px-2"
							>
								<Key />
							</div>
							<div
								class="w-full flex flex-col justify-center items-start pl-2 max-w-full overflow-hidden text-ellipsis whitespace-nowrap"
							>
								<div class="text-base flex font-semibold">
									<Highlight text="{result.name}" {query} />&nbsp;
									<span>in folder</span>&nbsp;
									<Highlight text="{result.folderName}" {query} />
								</div>
								<div class="text-sm font-normal">
									<Highlight text="{result.domain}" {query} />
								</div>
							</div>
							<div class="flex justify-center items-center">
								<LinkIcon />
							</div>
						</button>
					{/each}
				</div>
				{#if searchResults.length === 0}
					<div
						class="bg-osvauld-frameblack w-full flex justify-center items-center px-4 mb-4 py-0 sm:px-6 sm:flex sm:flex-row-reverse"
					>
						<p
							class="text-osvauld-placeholderblack text-base text-center w-[80%]"
						>
							Try searching for keywords in credentials, folders, groups,
							descriptions and more
						</p>
					</div>
				{/if}
			</div>
		</div>
	</div>
</button>
