<script lang="ts">
	import { slide, scale } from "svelte/transition";

	import { writeToClipboard } from "@osvauld/password-manager-common/utils/helper";
	import { credentialFieldsUpdater } from "@osvauld/password-manager-common/utils/credentialUtils";
	import LL from "@osvauld/password-manager-common/i18n/i18n-svelte";

	import Tick from "@osvauld/password-manager-common/icons/tick.svelte";
	import CopyIcon from "@osvauld/password-manager-common/icons/copyIcon.svelte";
	import ClosedEye from "@osvauld/password-manager-common/icons/closedEye.svelte";
	import Eye from "@osvauld/password-manager-common/icons/eye.svelte";
	import { onMount } from "svelte";

	export let type;
	export let fields;
	let copied = false;
	let copiedItemIndex = null;
	let showSecretIndex = {};

	const copyToClipboard = async (fieldValue, index) => {
		copiedItemIndex = index;
		copied = true;

		writeToClipboard(fieldValue);
		setTimeout(() => {
			copied = false;
			copiedItemIndex = null;
		}, 1000);
	};

	const toggleSecretVisibility = (fieldIndex) => {
		showSecretIndex = {
			...showSecretIndex,
			[fieldIndex]: !showSecretIndex[fieldIndex],
		};
	};
</script>

<div
	class="mt-3 bg-mobile-bgSeconary
 rounded-lg transition-all duration-300 flex flex-col items-start"
	in:slide
	out:slide>
	{#if type === "Note"}
		<!-- If credential type is note -->
		{@const value = fields.find(
			(item) => item.fieldName === "Note",
		)?.fieldValue}
		<div class="w-full flex flex-col gap-4">
			<div
				class="bg-osvauld-fieldActive rounded-lg py-2.5 px-4 w-full min-h-28 h-auto max-h-[15rem]">
				<p
					class="text-left text-mobile-textField text-base text-wrap break-all line-clamp-[9]">
					{value || "Regret"}
				</p>
			</div>
			<button
				class="bg-osvauld-fieldActive rounded-lg p-3 w-full flex gap-2 justify-center items-center"
				on:click|preventDefault|stopPropagation="{() =>
					copyToClipboard(value, 0)}">
				<span class="text-osvauld-fieldText font-Jakarta font-medium text-lg"
					>{$LL.copyNote()}</span>
				{#if copied && copiedItemIndex === 0}
					<span in:scale>
						<Tick />
					</span>
				{:else}
					<CopyIcon color="{'#85889C'}" />
				{/if}
			</button>
		</div>
	{:else}
		{#each fields as field, index (field.fieldName)}
			{#if field.fieldValue.trim().length !== 0}
				<span class="text-osvauld-fadedCancel text-sm"
					>{$LL.fieldNames[field?.fieldName]() || field.fieldName}</span>
				<div class="flex gap-2 w-full">
					<!-- {#if sensitiveFieldNames.includes(field.fieldName)} -->
					{#if credentialFieldsUpdater(type).find((templateField) => templateField.fieldName === field.fieldName)?.sensitive}
						<!-- If field name is sensitive, Hide -->

						<div
							class="bg-osvauld-fieldActive rounded-lg py-1 px-4 w-full flex"
							on:click|stopPropagation>
							<input
								type="{showSecretIndex[index] ? 'text' : 'password'}"
								class="w-5/6 text-left p-0 leading-3 bg-osvauld-fieldActive text-mobile-textField text-base border-0 outline-0 focus:ring-0 truncate"
								value="{showSecretIndex[index]
									? field.fieldValue
									: '••••••••'}" />
							<button
								type="button"
								class="ml-auto flex-none flex justify-center items-center"
								on:click="{() => toggleSecretVisibility(index)}">
								{#if showSecretIndex[index]}
									<ClosedEye />
								{:else}
									<Eye />
								{/if}
							</button>
						</div>
						<button
							class="bg-osvauld-fieldActive rounded-lg p-3"
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
					{:else}
						<!-- Regular credential field -->
						<div
							class="bg-osvauld-fieldActive rounded-lg py-2.5 px-4 flex-1 max-w-full truncate">
							<span class="text-left text-mobile-textField text-base"
								>{field.fieldValue}</span>
						</div>
						<button
							class="bg-osvauld-fieldActive rounded-lg p-3"
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
					{/if}
				</div>
			{/if}
		{/each}
	{/if}
</div>
