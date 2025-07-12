<script lang="ts">
	import { CopyIcon, Tick } from "@osvauld/password-manager-common";

  let { onLogin, collectedRecoveryString = $bindable() } = $props<{ onLogin: (isLoggedin: boolean) => void, collectedRecoveryString?: string }>();

  let revealKey = $state(false)
	let isCopied = $state(false)

	const handleLogin = () => {
		if(revealKey){
			onLogin(true);
		}else{
			revealKey = true;
		}
	}

	const skipReveal = () => {
		onLogin(true);
	}

</script>

<div class="relative pr-16 flex flex-col justify-center items-center">	<button class="absolute right-0 top-[13rem] cursor-pointer bg-osvauld-fieldActive border border-transparent focus:border-livnotePink outline-0 rounded-lg z-999 p-2.5" onclick={() => {
	navigator.clipboard.writeText(collectedRecoveryString)
	isCopied = true
	setTimeout(() => {
		isCopied = false
	}, 2000)
}}>
	{#if isCopied}<Tick/>{:else}	<CopyIcon color="#85889C"/>
	{/if}
</button>
	
<h1 class="text-xl font-semibold text-white mb-3 -mt-10">This is your recovery Key</h1>
<p class="text-sm font-inter font-extralight text-mobile-textActive mb-6 text-center">This helps you recover your account if you lose your password.<br/>you can find this in settlings later</p>
<div class="h-[343px] w-full sm:w-[600px] md:w-[800px] lg:w-[1000px] xl:w-[1173px] max-w-[1173px] text-osvauld-quarzowhite bg-osvauld-frameblack rounded-lg border border-osvauld-iconblack focus-within:border-livnotePink relative p-1.5 transition-colors duration-300 select-none">
	<div
		class="w-full max-w-full h-full border-0 tracking-wider font-light text-sm font-mono resize-none text-wrap scrollbar-thin overflow-y-scroll overflow-x-hidden p-1 outline-0 placeholder-placeholderGray break-all"
	>
		{collectedRecoveryString}
	</div>
	{#if !revealKey}
		<div 
			class="absolute inset-0 backdrop-blur-[3px] cursor-pointer rounded-md transition-all duration-300 "
		>
		</div>
	{/if}
</div>
<div class="flex gap-13 text-md font-medium mt-[9rem] select-none">
			<button
			class="w-[13.75rem] py-3.5 px-5 border border-mobile-bgHighlight text-mobile-textActive rounded-lg whitespace-nowrap cursor-pointer hover:text-white hover:border-mobile-textActive focus:border-livnotePink outline-0 transition-colors duration-300"
			onclick={skipReveal}
			>
			Not now
		</button>
		<button
		 onclick={handleLogin}
			disabled={!collectedRecoveryString}
			class="w-[13.75rem] py-3.5 px-5 bg-signupGray text-white rounded-md cursor-pointer border border-signupGray focus:border-livnotePink outline-0 transition-colors duration-300  enabled:hover:bg-livnotePink enabled:hover:text-mobile-bgPrimary">	
			{revealKey ? "Proceed":"Reveal my key"}
		</button>
</div>
</div>