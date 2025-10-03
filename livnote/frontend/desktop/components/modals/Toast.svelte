<script lang="ts">
	import { uiState } from "../../state/";
	import { fly } from "svelte/transition";
	import { ToastSuccess, ToastError } from "@osvauld/icons";
</script>

<div
	class="fixed h-[6rem] left-2 bottom-12 flex items-center justify-center bg-transparent font-light text-xl"
	aria-live="polite"
	in:fly={{ x: -300 }}
	out:fly={{ x: -300 }}
>
	<div
		class="flex justify-center items-center gap-1 rounded-lg border text-white {uiState
			.toastMessage.success
			? 'border-toastGreen '
			: 'border-osvauld-dangerRed'} font-normal text-xl pl-3 pr-5 py-2 bg-osvauld-cardshade"
	>
		{#if uiState.toastMessage.success}
			<ToastSuccess />
		{:else}
			<ToastError />
		{/if}
		<span class="text-base font-sans whitespace-nowrap">
			{uiState.toastMessage.message}!</span
		>
	</div>
</div>
