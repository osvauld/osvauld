<script lang="ts">
	let { onProceed, collectedRecoveryString = $bindable() } = $props();

	let errorMessage = $state(false);

	const handleProceed = () => {
		collectedRecoveryString = collectedRecoveryString.trim();

		// Validate private key format - must start with "-----BEGIN PRIVATE KEY-----"
		if (
			!collectedRecoveryString.startsWith(
				"-----BEGIN PGP PRIVATE KEY BLOCK-----",
			)
		) {
			errorMessage = true;
			setTimeout(() => {
				errorMessage = false;
			}, 3000);
			return;
		}

		//TODO: add additional validation to check if the private key is valid
		onProceed?.();
	};
</script>

<div
	class="h-[343px] w-[1173px] text-osvauld-quarzowhite bg-osvauld-frameblack rounded-lg border border-osvauld-iconblack focus-within:border-livnotePink relative p-1.5 transition-colors duration-300"
>
	<label for="privateKey" class="sr-only">Private Key Input</label>
	<textarea
		class="w-full h-full border-0 tracking-wider font-normal text-sm font-mono resize-none scrollbar-thin overflow-y-scroll p-1 outline-0 placeholder-placeholderGray"
		id="privateKey"
		name="privateKey"
		placeholder="Please Enter your private key"
		autocapitalize="off"
		autocomplete="off"
		aria-required="true"
		aria-invalid={errorMessage}
		bind:value={collectedRecoveryString}
		spellcheck="false"
		rows="8"
		onkeydown={(e) => {
			if (e.key === "Enter" || e.key === " ") {
				e.preventDefault();
				handleProceed();
			}
		}}
	></textarea>
</div>

<p
	class="text-red-400 text-sm font-normal mt-4"
	class:invisible={!errorMessage}
>
	Invalid private key format. Please enter a valid private key.
</p>

<div class="flex gap-13 text-md font-medium mt-[5rem] mb-10">
	<!-- <button
			class="py-3.5 px-5 border border-mobile-bgHighlight text-mobile-textActive rounded-lg whitespace-nowrap cursor-pointer hover:text-white hover:border-mobile-textActive focus:border-livnotePink outline-0 transition-colors duration-300"
			>
			I have lost my key
		</button> -->
	<button
		onclick={handleProceed}
		disabled={!collectedRecoveryString}
		class="w-[13.75rem] py-3.5 px-5 bg-signupGray text-white rounded-md cursor-pointer border border-signupGray focus:border-livnotePink outline-0 transition-colors duration-300 enabled:bg-livnotePink enabled:text-black"
	>
		Recover
	</button>
</div>
