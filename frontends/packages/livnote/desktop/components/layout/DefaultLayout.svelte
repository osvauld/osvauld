<script lang="ts">
	import NotesWorkspace from "./NotesWorkspace.svelte";
	import HeaderSection from "../layout/HeaderSection.svelte";
	import NavigationPanel from "../layout/NavigationPanel.svelte";
	import { onMount } from "svelte";
	import { vaults } from "../../store/desktop.ui.store";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
	import { listen, emit } from "@tauri-apps/api/event";
	import { noteViewLayout } from "../../store/desktop.ui.store";
	import { mergeDocuments } from "../notes/documentUtils.ts";
	let unsubscribeResourceUpdate: Function | null = null;
	onMount(async () => {
		try {
			console.log("default layout mounted");
			const resp = await sendMessage("getFolder");
			const updatedVaults = [{ id: "all", name: "All Vaults" }, ...resp];
			vaults.set(updatedVaults);

			unsubscribeResourceUpdate = await listen(
				"merge-update",
				async (event) => {
					console.log(event);
					let mergedDocument = mergeDocuments(
						event.payload.local_resource,
						event.payload.remote_resource,
					);
					emit("merge-complete", {
						mergedDocument,
						deviceId: event.payload.device_id,
						userId: event.payload.user_id,
						vectorClock: event.payload.vector_clock,
						resourceId: mergedDocument.resource_id,
					});
					console.log(mergedDocument);
				},
			);
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
