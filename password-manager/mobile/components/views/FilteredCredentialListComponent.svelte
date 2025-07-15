<script lang="ts">
	import MobileLeftArrow from "@osvauld/password-manager-common/icons/mobileLeftArrow.svelte";
	import MobileHome from "@osvauld/password-manager-common/icons/mobileHome.svelte";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
	import CredentialList from "./CredentialList.svelte";
	import {
		currentVault,
		credentialListWithType,
		favoriteCredentials,
	} from "../../store/mobile.ui.store";
	import { onMount } from "svelte";

	let credentials = [];

	const fetchCredentials = async () => {
		if ($favoriteCredentials) {
			credentials = await sendMessage("getAllCredentials", {
				favourite: true,
			});
			return;
		} else if ($currentVault.id === "all") {
			credentials = await sendMessage("getAllCredentials", {
				favourite: false,
			});
			return;
		} else {
			credentials = await sendMessage("getCredentialsForFolder", {
				folderId: $currentVault.id,
			});
		}
	};

	const goBack = () => {
		if ($favoriteCredentials) {
			favoriteCredentials.set(false);
		}
		credentialListWithType.set("");
	};

	onMount(() => {
		fetchCredentials();
	});
</script>

<nav class="w-full h-[48px] px-3 flex items-center gap-2 flex-shrink-0">
	<button on:click="{goBack}" class="p-2.5 rounded-lg bg-mobile-bgSeconary">
		<MobileLeftArrow />
	</button>
</nav>
<div
	class="text-mobile-textPrimary text-2xl font-medium flex flex-col pl-4 py-3">
	<span class="text-mobile-textTertiary">{$credentialListWithType}</span>
	<div
		class="flex text-sm font-light gap-1 items-center tracking-wider capitalize">
		<span><MobileHome size="{14}" color="{'#85889C'}" /></span>
		{$currentVault.name}
	</div>
</div>
{#if credentials.length !== 0}
	<CredentialList {credentials} />
{:else}
	<span
		class="w-full h-full flex justify-center items-center text-md text-white"
		>No credentials found</span>
{/if}
