<script lang="ts">


let { onProceed, collectedUsername = $bindable() } = $props<{ onProceed: () => void, collectedUsername?: string }>();	


const handleSubmit = (e: Event) => {
	e.preventDefault();
	if (collectedUsername.trim()) {
		collectedUsername = collectedUsername.trim();
		onProceed();
	}
};

</script>


<form onsubmit={handleSubmit} class="h-full flex flex-col items-center justify-center">
	<h1 class="text-xl font-semibold text-white mb-3 -mt-10">Add your name</h1>
	<p class="text-sm font-inter font-extralight text-mobile-textActive mb-14 text-center">Only seen by people you share something with. <br/>There is no central registry for these names.</p>
	<div class="h-[20rem] mt-6">
		<label for="username" class="font-normal mb-2 text-white self-start block">Enter Username</label>
		<div
			class="w-[24rem] flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-livnotePink ">
			<input
				class="w-full h-[3.3rem] text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal focus:ring-0 focus:focus:outline-none"
				type="text"
				id="username"
				aria-required="true"
				required
				autocomplete="off"
				autocorrect="off"
				bind:value={collectedUsername} />
		</div>
	</div>
	<button
		class="w-[24rem] py-2 px-10 mt-8 rounded-lg font-medium flex justify-center items-center whitespace-nowrap cursor-pointer border border-signupGray focus:border-livnotePink outline-0 transition-colors duration-300 enabled:hover:bg-livnotePink enabled:hover:text-mobile-bgPrimary"
		class:bg-livnotePink={collectedUsername.trim().length >= 4}
		class:text-mobile-bgPrimary={collectedUsername.trim().length >= 4}
		class:bg-signupGray={collectedUsername.trim().length < 4}
		class:text-white={collectedUsername.trim().length < 4}
		type="submit"
		disabled={!collectedUsername}
		onclick={handleSubmit}>
		<span>Next</span>
	</button>

</form>