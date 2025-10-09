<script lang="ts">


let { onProceed, collectedUsername = $bindable() } = $props<{ onProceed: () => void, collectedUsername?: string }>();	


const handleSubmit = (e: Event) => {
	e.preventDefault();
	if (collectedUsername?.trim()) {
		collectedUsername = collectedUsername.trim();
		onProceed();
	}
};

const isUsernameValid = () => {
	return collectedUsername?.trim().length >= 4;
};

</script>


<form onsubmit={handleSubmit} class="h-full flex flex-col items-center justify-around py-10">
	<div class="flex flex-col items-center justify-center mb-4">
		<h1 class="text-xl font-semibold text-white">Add your name</h1>
		<p class="text-sm font-inter font-extralight text-mobile-textActive  text-center">Only seen by people you share something with. <br/>There is no central registry for these names.</p>
    </div>
	<div class="mb-4">
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
		class="w-[24rem] py-2 px-10 rounded-lg font-medium flex justify-center items-center whitespace-nowrap cursor-pointer border border-signupGray focus:border-livnotePink outline-0 transition-colors duration-300"
		class:bg-livnotePink={isUsernameValid()}
		class:text-black={isUsernameValid()}
		class:bg-signupGray={!isUsernameValid()}
		class:text-white={!isUsernameValid()}
		type="submit"
		disabled={!collectedUsername?.trim()}>
		<span>Next</span>
	</button>

</form>