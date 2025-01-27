<script lang="ts">
	import MobileLeftArrow from "../../../icons/mobileLeftArrow.svelte";
	import MobileHome from "../../../icons/mobileHome.svelte";
	import { sendMessage } from "../../../lib/components/dashboard/helper";
	import CredentialList from "./CredentialList.svelte";
	import {
		currentVault,
		credentialListWithType,
	} from "../../store/mobile.ui.store";
	import { onMount } from "svelte";

	let credentials = [];

	const fetchCredentials = async () => {
		if ($currentVault.id === "all") {
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
		credentialListWithType.set("");
	};

	onMount(() => {
		fetchCredentials();
	});
</script>

{#if credentials.length !== 0}
	<nav class="w-full h-[48px] px-3 flex items-center gap-2 flex-shrink-0">
		<button on:click="{goBack}" class="p-2.5 rounded-lg bg-mobile-bgSeconary">
			<MobileLeftArrow />
		</button>
	</nav>
	<div
		class="text-mobile-textPrimary text-2xl font-medium flex flex-col pl-4 py-3">
		<span class="text-mobile-textTertiary">{$credentialListWithType}</span>
		<div class="flex text-sm font-light gap-1 items-center tracking-wider">
			<span><MobileHome size="{14}" color="{'#85889C'}" /></span>
			{$currentVault.name}
		</div>
	</div>

	<CredentialList {credentials} />
{/if}
