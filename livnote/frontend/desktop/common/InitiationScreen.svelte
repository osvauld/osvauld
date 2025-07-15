<script lang="ts">

	let { onFlowSelect } = $props<{
		onFlowSelect: (isImport: boolean) => void;
	}>();
</script>


	<div class="grow flex flex-col items-center justify-around pt-16 select-none">
		<h1 class="font-extralight mb-4 font-Jakarta text-7xl text-center text-white">
			Write, Connect & Collaborate <br/> <span class="text-livnotePink">without</span> servers. 
			<br />
			<span class="text-mobile-textActive text-[76px]">
				Encrypted, open & free
			</span>
		</h1>
		<div class="flex gap-13 text-md font-medium">
			<button
				class="py-3.5 px-5 border border-mobile-bgHighlight text-mobile-textActive rounded-lg whitespace-nowrap cursor-pointer hover:text-white hover:border-mobile-textActive transition-colors duration-300"
				onclick={() => onFlowSelect(true)}>
				I already have the key
			</button>
			<button
				onclick={() => onFlowSelect(false)}
				class="w-[13.75rem] py-3.5 px-5 bg-signupGray text-white rounded-md cursor-pointer transition-colors duration-300 hover:bg-livnotePink hover:text-mobile-bgPrimary">
				I am new here
			</button>
		</div>
		<p class="font-inter text-disclaimerGray text-center text-sm">
			By continuing you agree to our Terms of Use and Privacy Policy
		</p>
	</div>