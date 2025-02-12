<script lang="ts">
	import { onMount } from "svelte";
	import { toastStore } from "../store/desktop.ui.store";
	// let closehovered = false;
	const triggerCloseEvent = () => {
		toastStore.set({ show: false, message: "", success: false });
	};
	onMount(() => {
		setTimeout(() => {
			triggerCloseEvent();
		}, 2000);
	});
</script>

<div
	class="fixed h-[6rem] right-20 bottom-6 flex items-center justify-center px-6 py-5 my-2 rounded-lg bg-transparent font-normal text-xl">
	<div
		class="flex justify-center items-center rounded-xl border {$toastStore.success
			? 'border-osvauld-iconblack text-osvauld-chalkwhite '
			: 'border-osvauld-dangerRed text-osvauld-dangerRed'} font-normal text-xl px-5 py-4 my-2 bg-osvauld-cardshade">
		<span class="text-base font-sans whitespace-nowrap">
			{$toastStore.message}!</span>
	</div>
</div>
