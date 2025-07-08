<script lang="ts">
	import { sendMessage } from "../../../common/utils/helper";
	import NewPassword from "./NewPassword.svelte";

	let { recoveryData = "", onLogin } = $props();

	const handleInputChange = (event: any) => {
		recoveryData = event.target.value;
	};

	const handleSubmit = async (e: any) => {
		const passphrase = e.detail.passphrase;
		let recovery = JSON.parse(recoveryData);
		const result = await sendMessage("addDevice", {
			passphrase,
			certificate: recovery.certificate,
		});
		await sendMessage("login", { passphrase });
		console.log("sending first device connect message");
		await sendMessage("firstDeviceConnect", { ticket: recovery.ticket });

		onLogin?.(true);
	};

	const doSomething = () => {

	}

</script>

 <div class="h-[343px] w-[1173px] text-osvauld-quarzowhite bg-osvauld-frameblack rounded-lg border border-osvauld-iconblack focus-within:border-livnotePink relative p-1.5 transition-colors duration-300">
	<textarea
		class="w-full h-full border-0 tracking-wider font-light text-sm font-mono resize-none  scrollbar-thin overflow-y-scroll p-1 outline-0 placeholder-placeholderGray"
		id="privateKey"
		value={recoveryData}
		placeholder="Please Enter your private key"
		oninput={handleInputChange}></textarea>
	</div>
	<div class="flex gap-13 text-md font-medium mt-[9rem]">
			<button
			class="py-3.5 px-5 border border-mobile-bgHighlight text-mobile-textActive rounded-lg whitespace-nowrap cursor-pointer hover:text-white hover:border-mobile-textActive focus:border-livnotePink outline-0 transition-colors duration-300"
			onclick={() => doSomething(true)}>
			I have lost my key
		</button>
		<button
			onclick={() => doSomething(false)}
			class="w-[13.75rem] py-3.5 px-5 bg-signupGray text-white rounded-md cursor-pointer hover:bg-livnotePink hover:text-mobile-bgPrimary border border-signupGray focus:border-livnotePink outline-0 transition-colors duration-300">
			Proceed
		</button>
	</div>
	<!-- <NewPassword onSubmit={handleSubmit} /> -->
