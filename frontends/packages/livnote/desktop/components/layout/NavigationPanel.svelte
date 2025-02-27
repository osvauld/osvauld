<script lang="ts">
	import Arrow from "@osvauld/password-manager-common/icons/rightArrow.svelte";
	import Home from "@osvauld/password-manager-common/icons/mobileHome.svelte";
	import Star from "@osvauld/password-manager-common/icons/star.svelte";
	import VaultManager from "../ui/VaultManager.svelte";
	import {
		currentVault,
		noteViewLayout,
		noteId,
	} from "../../store/desktop.ui.store";
	import { CATEGORIES } from "@osvauld/password-manager-common/utils/credentialUtils";
	import { LL } from "@osvauld/password-manager-common/i18n/i18n-svelte";
	import { LocalStorageService } from "@osvauld/password-manager-common";
	import { StorageService } from "@osvauld/password-manager-common";
	import MobileNote from "@osvauld/password-manager-common/icons/mobileNote.svelte";
	import FavStar from "@osvauld/password-manager-common/icons/favStar.svelte";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
	import { onMount } from "svelte";

	let selectedSection = "home";
	let localSelectedCredential = 0;
	let vaultManagerActive = false;
	let hoveredCredential = null;

	let credentials = [];
	let isLoading = false;

	// Function to extract title from content
	function extractTitle(content) {
		if (!content) return "Untitled Note";

		// Try to find a heading tag
		const headingMatch = content.match(/<heading[^>]*>(.*?)<\/heading>/);
		if (headingMatch && headingMatch[1]) {
			return headingMatch[1].replace(/<[^>]+>/g, "").trim();
		}

		// Otherwise, get the first paragraph or line
		const firstParagraphMatch = content.match(
			/<paragraph[^>]*>(.*?)<\/paragraph>/,
		);
		if (firstParagraphMatch && firstParagraphMatch[1]) {
			const text = firstParagraphMatch[1].replace(/<[^>]+>/g, "").trim();
			// Return first 30 chars if there's text
			return text
				? text.length > 30
					? text.substring(0, 30)
					: text
				: "Untitled Note";
		}

		return "Untitled Note";
	}

	// Async function to fetch credentials based on vault ID
	async function fetchCredentials(vaultId) {
		isLoading = true;
		try {
			let fetchedCredentials;
			if (vaultId === "all") {
				fetchedCredentials = await sendMessage("getAllCredentials", {
					favourite: false,
				});
			} else {
				fetchedCredentials = await sendMessage("getCredentialsForFolder", {
					folderId: vaultId,
				});
			}

			// Filter for notes only
			credentials = fetchedCredentials.filter(
				(cred) => cred.data && cred.data.content && cred.data.editor_state,
			);

			// Sort by last accessed/modified (most recent first)
			credentials.sort((a, b) => {
				const timeA = a.data.last_accessed || a.data.last_modified || 0;
				const timeB = b.data.last_accessed || b.data.last_modified || 0;
				return timeB - timeA;
			});
		} catch (error) {
			console.error("Error fetching credentials:", error);
			credentials = [];
		} finally {
			isLoading = false;
		}
	}

	// Function to handle note selection
	function selectNote(id) {
		noteId.set(id);
	}

	// Watch for changes to currentVault
	$: if ($currentVault && $currentVault.id) {
		// Call the async function when vault changes
		fetchCredentials($currentVault.id);

		// Store Current vault for persisting
		(async () => {
			try {
				const currentVaultString = JSON.stringify($currentVault);
				await StorageService.setCurrentVault(currentVaultString);
			} catch (error) {
				console.error("Error storing vault:", error);
			}
		})();
	}

	// Handle section changes
	function handleSectionChange(section) {
		selectedSection = section;
		// If you want to implement favorite filtering, you could do that here
		if (section === "favourites") {
			fetchFavorites();
		} else {
			fetchCredentials($currentVault.id);
		}
	}

	// Function to fetch favorites
	async function fetchFavorites() {
		isLoading = true;
		try {
			const allCredentials = await sendMessage("getAllCredentials", {
				favourite: true,
			});

			// Filter for notes only
			credentials = allCredentials.filter(
				(cred) => cred.data && cred.data.content && cred.data.editor_state,
			);

			// Sort by last accessed/modified (most recent first)
			credentials.sort((a, b) => {
				const timeA = a.data.last_accessed || a.data.last_modified || 0;
				const timeB = b.data.last_accessed || b.data.last_modified || 0;
				return timeB - timeA;
			});
		} catch (error) {
			console.error("Error fetching favorites:", error);
			credentials = [];
		} finally {
			isLoading = false;
		}
	}

	// Load initial data
	onMount(() => {
		if ($currentVault && $currentVault.id) {
			fetchCredentials($currentVault.id);
		}
	});
</script>

<nav
	class="w-[360px] h-full py-10 px-4 whitespace-nowrap"
	aria-label="Main Navigation">
	<div class="relative">
		<button
			class="w-full text-[26px] text-osvauld-fieldText font-medium leading-6 bg-osvauld-frameblack rounded-lg border border-osvauld-defaultBorder px-4 py-2 flex justify-between items-center capitalize trun"
			aria-label="Switch Vault"
			aria-controls="vaultSelector"
			aria-expanded="false"
			on:click="{() => (vaultManagerActive = !vaultManagerActive)}">
			<span class="flex-1 truncate text-left py-1"
				>{$currentVault.id === "all" ? "All Vaults" : $currentVault.name}</span
			><span
				class="shrink-0 transition-transform duration-300 {vaultManagerActive
					? '-rotate-90'
					: 'rotate-90'}"><Arrow color="#F2F2F0" size="{24}" /></span
			></button>
		{#if vaultManagerActive}
			<VaultManager bind:vaultManagerActive instance="nav" />
		{/if}
	</div>
	<div
		class="border-b border-osvauld-borderColor text-osvauld-fieldText flex flex-col my-6 py-1 gap-1">
		<!-- <ul class="space-y-1 font-light text-base text-" role="list">
			<li>
				<button
					class="w-full flex items-center gap-3 p-3 rounded-lg
                       transition-colors
                       {!$noteId && selectedSection === 'home'
						? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
						: ''}"
					on:click="{() => handleSectionChange('home')}"
					aria-current="{selectedSection === 'home' ? 'page' : undefined}">
					<Home
						color="{!$noteId && selectedSection === 'home'
							? '#F2F2F0'
							: '#85889C'}" />
					<span>Home</span>
				</button>
			</li>
			<li>
				<button
					class="w-full flex items-center gap-3 p-3 rounded-lg
                       {!$noteId && selectedSection === 'favourites'
						? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
						: ''}"
					on:click="{() => handleSectionChange('favourites')}"
					aria-current="{selectedSection === 'favourites'
						? 'page'
						: undefined}">
					<Star
						color="{!$noteId && selectedSection === 'favourites'
							? '#F2F2F0'
							: '#85889C'}" />
					<span>Favourites</span>
				</button>
			</li>
		</ul> -->
	</div>

	{#if isLoading}
		<div class="text-osvauld-fieldText text-center p-4">Loading...</div>
	{:else}
		<ul
			class="font-light text-base space-y-1 text-osvauld-fieldText max-h-3/4 overflow-y-scroll px-1 scrollbar-thin"
			role="list">
			{#each credentials as note (note.id)}
				{@const hoveredOrSelected =
					hoveredCredential === note.id || $noteId === note.id}
				<li>
					<button
						class="w-full flex items-center justify-between gap-3 p-3 rounded-lg
							transition-colors
							{hoveredOrSelected
							? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
							: ''}"
						on:mouseenter="{() => (hoveredCredential = note.id)}"
						on:mouseleave="{() => (hoveredCredential = null)}"
						on:click="{() => selectNote(note.id)}">
						<div class="flex items-center gap-3 truncate">
							<span class="shrink-0">
								<MobileNote
									color="{hoveredOrSelected ? '#F2F2F0' : '#85889C'}" />
							</span>
							<span class="truncate">
								{note.data && note.data.content
									? extractTitle(note.data.content)
									: note.id}
							</span>
						</div>
						<span class="flex-shrink-0">
							{#if note.favourite}
								<FavStar />
							{:else}
								<Star color="{hoveredOrSelected ? '#F2F2F0' : '#85889C'}" />
							{/if}
						</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</nav>
