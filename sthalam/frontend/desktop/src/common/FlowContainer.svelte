<script lang="ts">
	import { fade } from "svelte/transition";
	import { GoBack } from "@osvauld/icons";

	let { onBack, children } = $props<{
		onBack: () => void;
		children: () => unknown;
	}>();

	let goBackHover = $state(false);
</script>

<div
	in:fade={{ duration: 300 }}
	class="grow flex flex-col justify-center items-center"
>
	<button
		class="absolute top-16 left-6 cursor-pointer border border-transparent focus:border-livnotePink outline-0 rounded-lg p-1"
		onmouseenter={() => (goBackHover = true)}
		onmouseleave={() => (goBackHover = false)}
		onclick={onBack}
	>
		<GoBack color={goBackHover ? "#fff" : "#2F303E"} />
	</button>
	{@render children()}
</div>
