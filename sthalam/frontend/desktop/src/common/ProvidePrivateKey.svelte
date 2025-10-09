<script lang="ts">
	import { CopyIcon, Tick } from "@osvauld/icons";
	import { writeToClipboard } from "../utils/helper";

	let { onLogin, collectedRecoveryString = $bindable() } = $props<{
		onLogin: (isLoggedin: boolean) => void;
		collectedRecoveryString?: string;
	}>();

	let revealKey = $state(false);
	let isCopied = $state(false);

	const handleLogin = () => {
		if (revealKey) {
			onLogin(true);
		} else {
			revealKey = true;
		}
	};

	const skipReveal = () => {
		onLogin(true);
	};
</script>

<div class="h-full w-full flex flex-col items-center justify-around py-10">
	<div class="flex flex-col items-center justify-center mb-4">
		<h1 class="text-xl font-semibold text-white">This is your recovery Key</h1>
		<p
			class="text-sm font-inter font-extralight text-mobile-textActive text-center"
		>
			This helps you recover your account if you lose your password.<br />you
			can find this in settings later.
		</p>
	</div>
	<div class="relative w-full my-10">
		<div
			class="h-[343px] text-osvauld-quarzowhite bg-osvauld-frameblack rounded-lg border border-osvauld-iconblack focus-within:border-livnotePink relative p-1.5 transition-colors duration-300"
		>
			<div
				class="w-full max-w-full h-full border-0 tracking-wider font-light text-sm font-mono resize-none text-wrap scrollbar-thin overflow-y-scroll overflow-x-hidden p-1 outline-0 placeholder-placeholderGray break-all"
			>
				{collectedRecoveryString}
			</div>
			{#if !revealKey}
				<div
					class="absolute inset-0 bg-black/50 rounded-md cursor-pointer backdrop-blur-[3px] transition-all duration-300"
					style="-webkit-backdrop-filter: blur(3px); backdrop-filter: blur(3px);"
				></div>
			{/if}
		</div>
		<button
			class="absolute right-2 top-2 cursor-pointer bg-osvauld-fieldActive border border-transparent focus:border-livnotePink outline-0 rounded-lg z-999 p-2.5"
			onclick={async () => {
				await writeToClipboard(collectedRecoveryString);
				isCopied = true;
				setTimeout(() => {
					isCopied = false;
				}, 2000);
			}}
		>
			{#if isCopied}<Tick />{:else}
				<CopyIcon color="#85889C" />
			{/if}
		</button>
	</div>
	<div class="flex gap-4">
		<button
			class="w-[12rem] h-12 px-5 border border-mobile-bgHighlight text-mobile-textActive rounded-lg whitespace-nowrap cursor-pointer hover:text-white hover:border-mobile-textActive focus:border-livnotePink outline-0 transition-colors duration-300"
			onclick={skipReveal}
		>
			Not now
		</button>
		<button
			onclick={handleLogin}
			disabled={!collectedRecoveryString}
			class="w-[12rem] h-12 px-5 rounded-lg font-medium flex justify-center items-center whitespace-nowrap cursor-pointer border border-signupGray focus:border-livnotePink outline-0 transition-colors duration-300 bg-livnotePink text-black"
		>
			{revealKey ? "Proceed" : "Reveal my key"}
		</button>
	</div>
</div>
