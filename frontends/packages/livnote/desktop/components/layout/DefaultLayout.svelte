<script lang="ts">
	import NotesWorkspace from "./NotesWorkspace.svelte";
	import HeaderSection from "../layout/HeaderSection.svelte";
	import NavigationPanel from "../layout/NavigationPanel.svelte";
	import { onMount } from "svelte";
	import { vaults } from "../../store/desktop.ui.store";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";

	import { noteViewLayout } from "../../store/desktop.ui.store";

	onMount(async () => {
		try {
			console.log("default layout mounted");
			const resp = await sendMessage("getFolder");
			const updatedVaults = [{ id: "all", name: "All Vaults" }, ...resp];
			vaults.set(updatedVaults);
			let connectionTicket = "";
			let certificate = "";
			let recoveryString = "";
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
			console.log(recoveryString);
		} catch (e) {
			console.log("Error received ===>", e);
		}
	});
</script>

<div class="w-full h-full bg-osvauld-ninjablack flex flex-col overflow-hidden">
	<HeaderSection />
	<div class="grow flex overflow-hidden">
		{#if $noteViewLayout}
			<NavigationPanel />
		{/if}
		<NotesWorkspace />
	</div>
</div>
