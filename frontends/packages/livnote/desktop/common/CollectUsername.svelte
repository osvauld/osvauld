<script lang="ts">

let collectedUsername = $state("");

</script>


<div class="h-full flex flex-col items-center justify-center">
	<label for="username" class="font-normal mt-6 mb-2 text-osvauld-quarzowhite">Enter Username</label>
  <div
		class="w-[24rem] flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-osvauld-activeBorder">
		<input
			class="text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent focus:ring-0 focus:border-osvauld-activeBorder focus:outline-none"
			type="text"
			id="username"
			aria-label="Username input"
			aria-required="true"
			aria-describedby="username-desc"
			required
			autocomplete="off"
			autocorrect="off"
			bind:value={collectedUsername} />
	</div>
	<p id="username-desc" class="sr-only">Please enter your username. This field is required to create your account.</p>
	<button
		class="w-full  py-2 px-10 mt-[8rem] rounded-lg font-medium  flex justify-center items-center whitespace-nowrap cursor-pointer bg-signupGray text-white  border border-signupGray focus:border-livnotePink outline-0 transition-colors duration-300  enabled:hover:bg-livnotePink enabled:hover:text-mobile-bgPrimary"
		type="submit"
		disabled={!collectedUsername}>
			<span>Submit</span>	
	</button>

</div>