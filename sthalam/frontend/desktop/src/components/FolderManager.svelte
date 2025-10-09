<script lang="ts">
	import { slide, fly } from "svelte/transition";
	import { Add, MobileHome, FolderIcon } from "@osvauld/icons";
	import { sendMessage } from "../utils/helper";
	import { dataState, uiState } from "../state";
	import type { Website } from "../types";

	let newWebsiteInputActive = $state(false);
	let newWebsiteName = $state("");
	let { position = "noteList" }: { position: "navigationPanel" | "noteList" } =
		$props();
	let isCreationDisabled = $derived(newWebsiteName.trim().length === 0);

	const autofocus = (node: HTMLElement) => {
		node.focus();
	};

	const handleWebsiteCreation = async (event: Event) => {
		// Stop event propagation to prevent the modal from closing
		event.stopPropagation();

		// Prevent the default form submission behavior
		event.preventDefault();

		try {
			await sendMessage("addFolder", {
				name: newWebsiteName.trim(),
				description: "",
			});
			await dataState.fetchWebsites();
			const newWebsite = dataState.websites.find(
				(website) => website.name === newWebsiteName.trim(),
			);
			if (newWebsite) {
				dataState.switchWebsite(newWebsite);
			}
			// Reset form state reactively
			newWebsiteName = "";
			newWebsiteInputActive = false;
			uiState.toggleFolderManager();
		} catch (e) {
			console.log("Website creation failed", e);
		}
	};

	const handleWebsiteSwitch = (website: Website) => {
		dataState.switchWebsite(website);
		uiState.toggleFolderManager();
	};

	const handleNewWebsiteInput = (e: Event) => {
		e.preventDefault();
		e.stopPropagation();
		newWebsiteInputActive = !newWebsiteInputActive;
	};
</script>

<div
	class="fixed inset-0 bg-transparent z-[999]"
	role="presentation"
	onclick={() => uiState.toggleFolderManager()}
>
	<div
		class={`absolute w-[20rem] h-[18rem] overflow-hidden scrollbar-thin border border-osvauld-iconblack bg-osvauld-ninjablack rounded-2xl px-2 pt-2 pb-3 flex flex-col gap-2 text-lg ${position === "noteList" ? "top-40 left-4" : "top-40 left-4"}`}
		style="width: calc(360px - 2rem);"
		id="websiteSelector"
		in:fly
	>
		<div class="h-full flex flex-col">
			<div class="flex-1 overflow-y-auto space-y-2 scrollbar-thin p-1">
				{#each dataState.websites as website (website.id)}
					{@const isActive = dataState.currentWebsite.id === website.id}
					<button
						class="h-[48px] w-full p-4 text-mobile-textPrimary flex items-center rounded-lg hover:bg-osvauld-frameblack"
						class:bg-mobile-bgLight={isActive}
						class:text-osvauld-sideListTextActive={isActive}
						onclick={(e) => {
							e.stopPropagation();
							handleWebsiteSwitch(website);
						}}
					>
						<span>
							{#if website.id === "all"}
								<MobileHome color={isActive ? "#F2F2F0" : "#85889C"} />
							{:else}
								<FolderIcon color={isActive ? "#F2F2F0" : "#85889C"} />
							{/if}
						</span>
						<span class="grow text-left pl-2 capitalize max-w-full truncate"
							>{website.id === "all" ? "All Websites" : website.name}</span
						>
					</button>
				{/each}
			</div>
			<div class="p-2 bg-osvauld-ninjablack">
				{#if newWebsiteInputActive}
					<form
						class="rounded-[20px] border border-mobile-bgLight px-3 pt-3 pb-4 text-mobile-textPrimary flex flex-col gap-3"
						in:slide
						out:slide
						onsubmit={handleWebsiteCreation}
					>
						<div
							class="w-full h-full"
							role="none"
							onclick={(e) => e.stopPropagation()}
							onkeydown={(e) =>
								e.key === "Escape" && uiState.toggleFolderManager()}
						>
							<span class="text-sm text-center">New Website </span>
							<span class="w-full border-b border-osvauld-modalFieldActive"
							></span>
							<div class="flex flex-col grow gap-1">
								<label for="new-website-name" class="sr-only">Add Title</label>
								<input
									type="text"
									id="new-website-name"
									class="bg-mobile-bgSeconary p-2 border-0 outline-0 focus:ring-0 rounded-lg text-white placeholder:text-sm placeholder:text-osvauld-iconblack"
									placeholder="Title"
									autocomplete="off"
									autocorrect="off"
									use:autofocus
									bind:value={newWebsiteName}
								/>
								<button
									type="submit"
									class="h-[48px] flex justify-center items-center gap-1 rounded-lg mt-6 text-base cursor-pointer"
									class:bg-signupGray={isCreationDisabled}
									class:text-white={isCreationDisabled}
									class:bg-livnotePink={!isCreationDisabled}
									class:text-black={!isCreationDisabled}
									disabled={isCreationDisabled}
									>Create new website <Add
										color={isCreationDisabled ? "#fff" : "#000"}
									/></button
								>
							</div>
						</div>
					</form>
				{:else}
					<button
						onclick={handleNewWebsiteInput}
						class="h-[48px] w-full flex justify-center items-center gap-1 rounded-lg border-2 border-mobile-bgHighlight p-4 active:bg-mobile-bgLight text-mobile-textActive text-base cursor-pointer"
						>Create new website<Add color="#85889C" /></button
					>
				{/if}
			</div>
		</div>
	</div>
</div>
