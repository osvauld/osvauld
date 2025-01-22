<script lang="ts">
	import { onMount, onDestroy } from "svelte";
	import { sendMessage } from "../../../lib/components/dashboard/helper";
	import {
		currentVault,
		refreshCredentialList,
	} from "../../store/mobile.ui.store";
	import { listen, UnlistenFn } from "@tauri-apps/api/event";
	import RegretFace from "../../../icons/regretFace.svelte";
	import CategoriesFilter from "./CategoriesFilter.svelte";
	import CredentialList from "./CredentialList.svelte";

	let credentials = [];
	let isRecents = true;
	let credentialsLoaded = false;

	let unlisten: UnlistenFn;

	const fetchAllCredentials = async () => {
		try {
			credentials = await sendMessage("getAllCredentials", {
				favourite: false,
			});
			credentialsLoaded = true;
			console.log("fetching all credentials", credentials);
		} catch (error) {
			credentials = [];
		}
	};

	// This async function handles fetching and processing notes
	const fetchCredentials = async (vaultId: string) => {
		try {
			credentials = await sendMessage("getCredentialsForFolder", {
				folderId: vaultId,
			});

			credentialsLoaded = true;
		} catch (error) {
			credentials = [];
		}
	};

	// Setup event listener for sync-complete
	const setupSyncListener = async () => {
		try {
			unlisten = await listen("sync-completed", (event) => {
				if (event.payload === true && $currentVault?.id) {
					fetchCredentials($currentVault.id);
				}
			});
		} catch (error) {
			console.error("Error setting up sync listener:", error);
		}
	};

	const fetchCredentialsForVault = async (vaultId) => {
		if (!vaultId) return;
		vaultId === "all"
			? await fetchAllCredentials()
			: await fetchCredentials(vaultId);
	};

	// Watch for vault changes
	$: fetchCredentialsForVault($currentVault?.id);
	$: if ($refreshCredentialList) {
		fetchCredentialsForVault($currentVault.id);
		refreshCredentialList.set(false);
	}

	// Initial setup on mount
	onMount(() => {
		// Fetch all Credentials
		setupSyncListener();
	});

	// Cleanup on component destruction
	onDestroy(() => {
		if (unlisten) {
			unlisten();
		}
	});
</script>

<div class="w-full h-[40px] flex justify-between items-center px-4">
	<div class="w-full flex border-b border-mobile-bgHighlight">
		<button
			class="flex-1 h-[48px] text-center {isRecents
				? 'text-osvauld-carolinablue border-b-2 border-osvauld-carolinablue'
				: 'text-mobile-textSecondary'}"
			on:click="{() => (isRecents = true)}">
			Recents
		</button>
		<button
			class="flex-1 h-[48px] text-center {!isRecents
				? 'text-osvauld-carolinablue border-b-2 border-osvauld-carolinablue'
				: 'text-mobile-textSecondary'}"
			on:click="{() => (isRecents = false)}">
			Categories
		</button>
	</div>
</div>

{#if !credentialsLoaded}
	<div class="w-full h-full flex justify-center items-center">
		<span class="text-mobile-textPrimary">Loading...</span>
	</div>
{:else if credentials.length === 0}
	<div
		class="w-full h-full flex justify-center items-center text-mobile-textPrimary text-base font-light">
		<div class="flex flex-col justify-center items-center gap-4">
			<span>
				<RegretFace size="{64}" />
			</span>
			<span>Unable to find associated credentials</span>
		</div>
	</div>
{:else if isRecents}
	<CredentialList {credentials} />
{:else}
	<CategoriesFilter {credentials} />
{/if}
