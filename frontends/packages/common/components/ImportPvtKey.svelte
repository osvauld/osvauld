<script lang="ts">
	import { sendMessage } from "../utils/helper";
	import NewPassword from "./NewPassword.svelte";

	// Using $props for component properties
	let { onLogin, recoveryData = "" } = $props();

	const handleInputChange = (event: any) => {
		recoveryData = event.target.value;
	};

	const handleSubmit = async (e: any) => {
		const passphrase = e.detail.passphrase;
		let parsedData = JSON.parse(recoveryData);
		const result = await sendMessage("addDevice", {
			passphrase,
			certificate: parsedData.certificate,
			username: parsedData.username,
			device_id: parsedData.deviceId,
		});
		await sendMessage("login", { passphrase });
		console.log("sending first device connect message");

		onLogin?.(true);
	};
</script>

<div
	class="flex flex-col justify-center items-center text-osvauld-sheffieldgrey">
	<div>
		<h3 class="font-medium text-3xl mb-6 text-osvauld-ownerText">
			Account Recovery
		</h3>
	</div>
	<label for="privateKey" class="font-normal mt-6 mb-2"
		>Enter Recovery string</label>
	<textarea
		class="text-osvauld-quarzowhite bg-osvauld-frameblack border border-osvauld-iconblack tracking-wider font-light text-sm font-mono focus:border-osvauld-iconblack focus:ring-0 resize-none w-[300px] min-h-[6rem] max-h-[10rem] rounded-lg scrollbar-thin overflow-y-scroll"
		id="privateKey"
		value={recoveryData}
		oninput={handleInputChange}></textarea>
	<NewPassword submit={handleSubmit} />

	{#snippet additionalControls()}
		<!-- This is where additional controls will be rendered -->
	{/snippet}
</div>
