<script lang="ts">
	import { uiState } from "../../state/";
	import { fly } from "svelte/transition";
	import { ToastSuccess, ToastError } from "../../icons";
</script>



<div
	class="fixed h-[6rem] left-20 bottom-6 flex items-center justify-center bg-transparent font-normal text-xl"
	in:fly={{ x: -300 }}
	out:fly={{ x: -300 }}>
	<div
		class="flex justify-center items-center  gap-4 rounded-lg border text-white {uiState
			.toastMessage.success
			? 'border-toastGreen '
			: 'border-osvauld-dangerRed'} font-normal text-xl pl-3 pr-5 py-3.5 bg-osvauld-cardshade">
		{#if uiState
			.toastMessage.success}
			<ToastSuccess />
		{:else}
			<ToastError />
		{/if}
		<span class="text-base font-sans whitespace-nowrap">
			{uiState
			.toastMessage.message}!</span>
	</div>
</div>
