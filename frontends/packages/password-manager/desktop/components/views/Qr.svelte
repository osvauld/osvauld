<script lang="ts">
	import QRCode from "@castlenine/svelte-qrcode";
	import { onMount } from "svelte";
	import { sendMessage } from "../../../lib/components/dashboard/helper";
	let connectionTicket = "";
	let certificate = "";
	let recoveryString = "";
	onMount(async () => {
		await sendMessage("startP2PListner");
		connectionTicket = await sendMessage("getTicket");
		// TODO: change the passphrase to the actual password
		certificate = await sendMessage("exportCertificate", {
			passphrase: "test",
		});
		recoveryString = JSON.stringify({
			ticket: connectionTicket,
			certificate: certificate,
		});
		console.log({ connectionTicket, certificate });
	});
</script>

{#if recoveryString}
	<div class="mx-auto">
		<QRCode
			size="{512}"
			typeNumber="{40}"
			data="{JSON.stringify({
				ticket: connectionTicket,
				certificate: certificate,
			})}" />
	</div>
{/if}
<textarea class="font-bold bg-black">
	{recoveryString}
</textarea>
