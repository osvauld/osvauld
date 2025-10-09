<script lang="ts">
	let { onFlowSelect } = $props<{
		onFlowSelect: (isImport: boolean) => void;
	}>();
</script>

<div class="grow flex flex-col items-center select-none">
	<div class="h-full flex justify-center items-center">
		<h1
			class="font-normal mb-4 font-jakarta text-7xl text-center text-white"
		>
			Sovereign browsing and hosting<br />
			<span class="text-mobile-textActive text-[76px]">
				Free, secure, and private
			</span>
			<br />
			<span class="text-mobile-textActive text-[76px]">
				Build and share on your terms.
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
				class="w-[13.75rem] py-3.5 px-5 rounded-md cursor-pointer transition-colors duration-300 bg-livnotePink text-black"
			>
				I am new here
			</button>
		</div>
		<!-- <p class="font-inter text-disclaimerGray text-center text-sm">
					By continuing you agree to our <a href="https://www.osvauld.com/terms/" class="italic">Terms of Use</a> and  <a href="https://www.osvauld.com/privacy/" class="italic">Privacy Policy</a>
				</p> -->
	</div>
</div>
