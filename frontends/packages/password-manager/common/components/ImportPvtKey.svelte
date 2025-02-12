<script lang="ts">
	import { createEventDispatcher } from "svelte";
	import { sendMessage } from "../utils/helper";
	import NewPassword from "./NewPassword.svelte";
	import {
		scan,
		Format,
		requestPermissions,
	} from "@tauri-apps/plugin-barcode-scanner";

	const dispatch = createEventDispatcher();
	let recoveryData = "";
	let scanning = false;
	const handleInputChange = (event: any) => {
		recoveryData = event.target.value;
	};

	const scanQRCode = async () => {
		try {
			await requestPermissions();
			scanning = true;
			console.log("got permission");
			const result = await scan({
				windowed: false,
				formats: [Format.QRCode],
			});
			if (result && result.content) {
				recoveryData = JSON.parse(result.content);
			}
		} catch (err) {
			console.log(err);
		} finally {
			scanning = false;
		}
	};

	const handleSubmit = async (e: any) => {
		const passphrase = e.detail.passphrase;
		let recovery = JSON.parse(recoveryData);
		const result = await sendMessage("addDevice", {
			passphrase,
			...recovery,
		});
		const pubkey = await sendMessage("login", { passphrase });
		dispatch("login", true);
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
		on:input="{handleInputChange}"></textarea>
	<NewPassword on:submit="{handleSubmit}" />

	<button
		on:click="{scanQRCode}"
		class="px-4 py-2.5 bg-mobile-bgHighlight text-mobile-textPrimary rounded-lg font-medium">
		{scanning ? "Scanning..." : "Scan QR"}
	</button>
</div>
