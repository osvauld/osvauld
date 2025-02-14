<script lang="ts">
	import Menu from "@osvauld/password-manager-common/icons/Menu.svelte";
	import {
		currentLayout,
		credentialLayoutType,
		selectedCredential,
		credentialListWithType,
		currentVault,
	} from "../../store/mobile.ui.store";
	import {
		CATEGORIES,
		renderRelevantHeading,
	} from "@osvauld/password-manager-common/utils/credentialUtils";

	export let credentials;
	let updatedCredentials;

	$: {
		updatedCredentials =
			$credentialListWithType && $credentialListWithType !== "favourites"
				? credentials.filter(
						(credential) =>
							credential.data.credentialType === $credentialListWithType,
					)
				: credentials;

		if ($currentVault.id !== "all") {
			updatedCredentials = updatedCredentials.filter(
				(credential) => credential.folder_id === $currentVault.id,
			);
		}
	}
</script>

<div class="grid grid-cols-1 gap-3 p-4 overflow-y-scroll overflow-x-hidden">
	{#each updatedCredentials as credential}
		{@const credentialType = credential.data.credentialType}
		{@const categoryInfo = CATEGORIES.find(
			(item) => item.type === credentialType,
		)}
		<div
			class="p-3 bg-mobile-bgSeconary rounded-xl font-normal flex justify-start items-center gap-3"
			on:click={() => {
				selectedCredential.set(credential);
				credentialLayoutType.set("view");
				currentLayout.set("credential");
			}}>
			<span class="flex justify-center items-center p-2">
				<!-- <MobileKey /> -->
				<svelte:component this={categoryInfo.icon} color={"#BFC0CC"} />
			</span>
			<div class="flex flex-col">
				<h3
					class="text-base text-left text-mobile-textbright truncate max-w-[16rem]">
					{renderRelevantHeading(
						credential.data.credentialFields,
						credentialType,
						credential.id,
					)}
				</h3>
				<span class="text-xs text-left text-mobile-textPrimary"
					>{credentialType}
				</span>
			</div>
			<button
				class="ml-auto p-3 flex justify-center items-center"
				on:click|stopPropagation={() =>
					console.log("Propagation is not allowed here")}>
				<Menu />
			</button>
		</div>
	{/each}
</div>
