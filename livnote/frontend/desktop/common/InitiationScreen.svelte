<script lang="ts">
	let { onFlowSelect } = $props<{
		onFlowSelect: (isImport: boolean) => void;
	}>();
</script>

<div class="grow flex flex-col items-center select-none">
	<div class="h-full flex justify-center items-center">
		<h1
			class="font-extralight mb-4 font-Jakarta text-7xl text-center text-white"
		>
			Create and edit notes together<br />
			<span class="text-mobile-textActive text-[76px]">
				Live, offline-first, free
			</span>
			<br />
			<span class="text-mobile-textActive text-[76px]">
				and end to end encrypted.
			</span>
		</h1>
	</div>
	<div class="flex flex-col items-center justify-center gap-12 my-10">
		<div class="flex gap-13 text-md font-medium">
			<button
				class="py-3.5 px-5 border border-mobile-bgHighlight text-mobile-textActive rounded-lg whitespace-nowrap cursor-pointer hover:text-white hover:border-mobile-textActive transition-colors duration-300"
				onclick={() => onFlowSelect(true)}
			>
				I already have the key
			</button>
			<button
				onclick={() => onFlowSelect(false)}
				class="w-[13.75rem] py-3.5 px-5 text-white rounded-md cursor-pointer transition-colors duration-300 bg-livnotePinktext-black"
			>
				I am new here
			</button>
		</div>
		<!-- <p class="font-inter text-disclaimerGray text-center text-sm">
					By continuing you agree to our <a href="https://www.osvauld.com/terms/" class="italic">Terms of Use</a> and  <a href="https://www.osvauld.com/privacy/" class="italic">Privacy Policy</a>
				</p> -->
	</div>
</div>
