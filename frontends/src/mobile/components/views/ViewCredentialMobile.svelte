<script lang="ts">
	import { onMount } from "svelte";
	import { scale } from "svelte/transition";

	import { sendMessage } from "../../../lib/components/dashboard/helper";

	import MobileLeftArrow from "../../../icons/mobileLeftArrow.svelte";
	import Tick from "../../../icons/tick.svelte";
	import CopyIcon from "../../../icons/copyIcon.svelte";
	import Share from "../../../icons/FolderShare.svelte";
	import More from "../../../icons/more.svelte";
	import Edit from "../../../icons/editIcon.svelte";
	import MobileHome from "../../../icons/mobileHome.svelte";
	import Bin from "../../../icons/binIcon.svelte";
	import Star from "../../../icons/star.svelte";
	import FavStar from "../../../icons/favStar.svelte";

	import {
		currentLayout,
		bottomNavActive,
		selectedCredential,
		credentialLayoutType,
		currentVault,
		vaults,
	} from "../../store/mobile.ui.store";

	import { writeToClipboard } from "../../../lib/components/dashboard/helper";

	let copied = false;
	let copiedItemIndex;
	let isFavourite = $selectedCredential?.favourite || false;

	let vaultName = $vaults.find(
		(vault) => vault.id === $selectedCredential.folder_id,
	)?.name;

	const directToHome = () => {
		currentLayout.set("home");
		bottomNavActive.set(true);
		selectedCredential.set({});
		credentialLayoutType.set("addition");
	};

	const copyToClipboard = async (fieldValue, index) => {
		copiedItemIndex = index;
		copied = true;

		await writeToClipboard(fieldValue);
		setTimeout(() => {
			copied = false;
		}, 2000);
	};

	const toggleFavorite = async (id) => {
		isFavourite = !isFavourite;
		await sendMessage("toggleFav", { credentialId: id });
	};

	onMount(() => {
		bottomNavActive.set(false);
	});
</script>

<nav class="w-full h-[48px] px-3 flex items-center gap-2 flex-shrink-0">
	<button
		on:click="{directToHome}"
		class="p-2.5 rounded-lg bg-mobile-bgSeconary">
		<MobileLeftArrow />
	</button>
	<span class="ml-auto p-2.5 rounded-lg bg-mobile-bgSeconary"
		><Edit size="{24}" /></span>
	<span class="p-2.5 rounded-lg bg-mobile-bgSeconary"><Share /></span>
	<button
		class="p-2.5 bg-mobile-bgSeconary rounded-md flex justify-center items-center active:scale-95"
		on:click|stopPropagation="{() => toggleFavorite($selectedCredential.id)}">
		{#if isFavourite}
			<FavStar />
		{:else}
			<Star />
		{/if}
	</button>
	<span class="p-2.5 rounded-lg bg-mobile-bgSeconary"
		><More color="#85889C" /></span>
</nav>
<div
	class="text-mobile-textPrimary text-2xl font-medium flex flex-col pl-4 py-3">
	<span class="text-mobile-textTertiary"
		>{$selectedCredential.data.credentialType}</span>
	<div
		class="flex text-sm font-light gap-1 items-center tracking-wider capitalize">
		<span><MobileHome size="{14}" color="{'#85889C'}" /></span>
		{vaultName || $currentVault.name}
	</div>
</div>

<div
	class="grow p-4 text-mobile-textPrimary overflow-y-auto scrollbar-thin flex flex-col gap-2 mt-2">
	{#each $selectedCredential.data.credentialFields as field, index (field.fieldName)}
		{#if field.fieldValue.trim().length !== 0}
			<span class="text-mobile-textlabel text-sm">{field.fieldName}</span>
			<div class="flex gap-2">
				<div
					class="bg-mobile-bgSeconary rounded-lg py-2.5 px-4 flex-1 max-w-full truncate">
					<span class=" text-mobile-textField text-base"
						>{field.fieldValue}</span>
				</div>
				<button
					class="bg-mobile-bgSeconary rounded-lg p-3"
					on:click|preventDefault|stopPropagation="{() =>
						copyToClipboard(field.fieldValue, index)}">
					{#if copied && copiedItemIndex === index}
						<span in:scale>
							<Tick />
						</span>
					{:else}
						<CopyIcon color="{'#85889C'}" />
					{/if}
				</button>
			</div>
		{/if}
	{/each}

	<hr class="border border-mobile-bgSeconary my-3" />
	<div
		class="bg-mobile-bgSeconary rounded-lg h-[65px] p-3 text-mobile-textPrimary text-sm">
		Created at : <br />
		<span class="text-xs"
			>{new Date().toLocaleTimeString() +
				"," +
				new Date().toLocaleDateString()}</span>
	</div>
	<hr class="border border-mobile-bgSeconary my-3" />
	<div class="flex flex-col gap-3">
		<span class="px-5 py-2.5 flex justify-start items-center">
			<Star />
			<span class="ml-2">Add to favourites</span></span>
		<span class="px-5 py-2.5 flex justify-start items-center">
			<Edit />
			<span class="ml-2">Edit secret</span></span>
		<span class="px-5 py-2.5 flex justify-start items-center">
			<Share />
			<span class="ml-2">Share now</span></span>
		<span
			class="text-osvauld-dangerRed px-5 py-2.5 flex justify-start items-center">
			<Bin color="#FF6A6A" />
			<span class="ml-2">Delete</span></span>
	</div>
</div>
