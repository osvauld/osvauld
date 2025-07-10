<script lang="ts">
	// @ts-ignore: Image import for Svelte, handled by bundler
	import { GoBack } from "@osvauld/password-manager-common";
	let { onBack, children } = $props<{
		onBack: () => void;
		children: any;
	}>();
</script>

<div class="grow flex flex-col justify-center items-center">
		<button 
			class="absolute top-1/2 left-0 -translate-y-1/2 cursor-pointer border border-transparent focus:border-livnotePink outline-0 rounded-lg p-1" 
			onclick={onBack}>
			<GoBack/>
		</button>
	{@render children()}
</div> 	