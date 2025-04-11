<script lang="ts">
	import { onMount } from "svelte";
	import { uiState } from "../../state/";
</script>

<div
	class="fixed h-[6rem] right-20 bottom-6 flex items-center justify-center px-6 py-5 my-2 rounded-lg bg-transparent font-normal text-xl">
	<div
		class="flex justify-center items-center rounded-xl border {uiState
			.toastMessage.success
			? 'border-osvauld-iconblack text-osvauld-chalkwhite '
			: 'border-osvauld-dangerRed text-osvauld-dangerRed'} font-normal text-xl px-5 py-4 my-2 bg-osvauld-cardshade">
		<span class="text-base font-sans whitespace-nowrap">
			{uiState.toastMessage.message}!</span>
	</div>
</div>
