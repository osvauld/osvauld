<script lang="ts">
	import {
		renderRelevantHeading,
		CATEGORIES,
	} from "@osvauld/password-manager-common/utils/credentialUtils";

	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
	import { onMount, createEventDispatcher } from "svelte";
	import MenuVertical from "@osvauld/password-manager-common/icons/menuVertical.svelte";
	import Star from "@osvauld/password-manager-common/icons/star.svelte";
	import FavStar from "@osvauld/password-manager-common/icons/favStar.svelte";
	import CredentialOverview from "./CredentialOverview.svelte";
	import { refreshCredentialList } from "../../store/desktop.ui.store";

	import { LL } from "@osvauld/password-manager-common//i18n/i18n-svelte";

	export let credential;
	export let credentialcardstates = [];
	let favourite = credential?.favourite || false;

	const dispatch = createEventDispatcher();

	let type = credential.data.credentialType;
	let categoryInfo = CATEGORIES.find((item) => item.type === type);
	let typeIdForTranslation = CATEGORIES.find((item) => item.type === type)?.id;

	const dispatchDoubleClick = (credential) => {
		dispatch("dbl", credential);
	};

	const dispatchClick = (id) => {
		dispatch("clk", id);
	};

	const toggleFavorite = async (id) => {
		favourite = !favourite;
		await sendMessage("toggleFav", { credentialId: id });
		refreshCredentialList.set(true);
	};

	// onMount(() => {
	// 	console.log(credential.id, credential.favourite);
	// });
</script>

<div class="min-w-0">
	<div
		class="bg-osvauld-frameblack w-full border border-osvauld-cardBorder rounded-xl p-4"
		on:dblclick|stopPropagation="{() => dispatchDoubleClick(credential)}"
		on:click|stopPropagation="{() => dispatchClick(credential.id)}">
		<div class="flex items-center mb-4">
			<span
				class="flex justify-center items-center p-2.5 bg-osvauld-fieldActive rounded-md mr-3">
				{#if categoryInfo && categoryInfo.icon}
					<svelte:component this="{categoryInfo.icon}" color="{'#BFC0CC'}" />
				{:else}
					<span>!</span>
				{/if}
			</span>
			<button
				class="ml-auto p-2.5 bg-osvauld-fieldActive rounded-md flex justify-center items-center active:scale-95"
				on:click|stopPropagation="{() => toggleFavorite(credential.id)}">
				{#if favourite}
					<FavStar />
				{:else}
					<Star />
				{/if}
			</button>
			<button
				class="ml-3 p-2.5 bg-osvauld-fieldActive rounded-md flex justify-center items-center"
				on:click|stopPropagation="{() => {}}">
				<MenuVertical />
			</button>
		</div>
		<div class="flex flex-col">
			<h3
				class="font-Jakarta text-xl font-medium text-left text-osvauld-sideListTextActive truncate max-w-[16rem]">
				{renderRelevantHeading(
					credential.data.credentialFields,
					type,
					credential.id,
				)}
			</h3>
			<span class="text-sm text-left text-osvauld-fieldTextActive">
				{$LL.types[typeIdForTranslation]()}
			</span>
		</div>
		{#if credentialcardstates.find((item) => item.id === credential.id)?.show}
			<CredentialOverview {type} fields="{credential.data.credentialFields}" />
		{/if}
	</div>
</div>
