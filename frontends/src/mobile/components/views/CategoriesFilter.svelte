<script lang="ts">
	import { onMount } from "svelte";
	import { CATEGORIES } from "../../../utils/CredentialUtils";
	import { credentialListWithType } from "../../store/mobile.ui.store";
	export let credentials = [];

	let credentialTypeCount = {};

	const showFilteredCredentialList = (credentialType) => {
		if (!credentialTypeCount[credentialType]) return;
		credentialListWithType.set(credentialType);
	};

	$: {
		credentials.forEach((credential) => {
			const type = credential.data.credentialType;
			credentialTypeCount[type] ??= 0;
			credentialTypeCount[type] += 1;
		});
	}
</script>

<div class="my-3 text-base grid grid-cols-2 gap-3 p-3 text-mobile-textPrimary">
	{#each CATEGORIES as category (category.id)}
		<button
			class=" bg-mobile-bgSeconary rounded-lg p-3 h-[100px]
               active:bg-mobile-bgHighlight/10 transition-colors touch-manipulation"
			on:click|preventDefault|stopPropagation="{() =>
				showFilteredCredentialList(category.type)}">
			<div class="flex flex-col h-full items-start justify-center gap-1 p-3">
				{#if category.icon}
					<span class="flex justify-center items-center py-2">
						<svelte:component this="{category.icon}" color="{'#BFC0CC'}" />
					</span>
				{/if}
				<span class="text-center text-base leading-tight text-mobile-textbright"
					>{category.type}</span>
				<span class="text-xs"
					>{credentialTypeCount[category.type] || 0} items</span>
			</div>
		</button>
	{/each}
</div>
