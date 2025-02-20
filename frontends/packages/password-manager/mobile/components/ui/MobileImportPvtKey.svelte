<!-- mobile/components/MobileImportPvtKey.svelte -->
<script lang="ts">
	import BaseImportPvtKey from "@osvauld/password-manager-common/components/BaseImportPvtKey.svelte";
	import {
		scan,
		Format,
		requestPermissions,
	} from "@tauri-apps/plugin-barcode-scanner";

	let recoveryData = "";
	let scanning = false;

	const scanQRCode = async () => {
		try {
			await requestPermissions();
			scanning = true;
			const result = await scan({
				windowed: false,
				formats: [Format.QRCode],
			});
			if (result?.content) {
				recoveryData = result.content;
			}
		} catch (err) {
			console.error(err);
		} finally {
			scanning = false;
		}
	};
</script>

<BaseImportPvtKey bind:recoveryData on:login>
	<div slot="additional-controls">
		<button
			on:click={scanQRCode}
			class="px-4 py-2.5 bg-mobile-bgHighlight text-mobile-textPrimary rounded-lg font-medium">
			{scanning ? "Scanning..." : "Scan QR"}
		</button>
	</div>
</BaseImportPvtKey>

