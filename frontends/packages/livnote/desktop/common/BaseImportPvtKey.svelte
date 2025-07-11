<script lang="ts">

	let {  onProceed } = $props();

	let recoveryData = $state("")


	const handleProceed = () => {
		//TODO: add basic validation and parse username to store locally
		const trimmedRecoveryData = recoveryData.trim();
		onProceed?.(trimmedRecoveryData);
	}

</script>

 <div class="h-[343px] w-[1173px] text-osvauld-quarzowhite bg-osvauld-frameblack rounded-lg border border-osvauld-iconblack focus-within:border-livnotePink relative p-1.5 transition-colors duration-300">
	<label for="privateKey" class="sr-only">Private Key Input</label>
	<textarea
		class="w-full h-full border-0 tracking-wider font-light text-sm font-mono resize-none scrollbar-thin overflow-y-scroll p-1 outline-0 placeholder-placeholderGray"
		id="privateKey"
		name="privateKey"
		placeholder="Please Enter your private key"
		autocapitalize="off"
		autocomplete="off"
		aria-required="true"
		bind:value={recoveryData}
		spellcheck="false"
		rows="8"
	></textarea>
	</div>
	<div class="flex gap-13 text-md font-medium mt-[9rem]">
			<button
			class="py-3.5 px-5 border border-mobile-bgHighlight text-mobile-textActive rounded-lg whitespace-nowrap cursor-pointer hover:text-white hover:border-mobile-textActive focus:border-livnotePink outline-0 transition-colors duration-300"
			>
			I have lost my key
		</button>
		<button
			onclick={() => handleProceed()}
			disabled={!recoveryData}
			class="w-[13.75rem] py-3.5 px-5 bg-signupGray text-white rounded-md cursor-pointer border border-signupGray focus:border-livnotePink outline-0 transition-colors duration-300  enabled:hover:bg-livnotePink enabled:hover:text-mobile-bgPrimary">
			Proceed
		</button>
	</div>
		