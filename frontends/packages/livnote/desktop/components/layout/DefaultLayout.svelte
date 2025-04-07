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
	
	// Define types for the event payload and other data structures
	interface MergeUpdatePayload {
		local_resource: any;
		remote_resource: any;
		device_id: string;
		user_id: string;
		vector_clock: any;
	}
	
	interface Vault {
		id: string;
		name: string;
	}
	
	// Define type for merged document
	interface MergedDocument {
		resource_id: string;
		[key: string]: any;
	}
	
	onMount(async () => {
		try {
			const resp: any[] = await sendMessage("getFolder");
			
			// Create a properly typed array by spreading any[] response into a typed array
			const defaultVault: Vault = { id: "all", name: "All Vaults" };
			const folderVaults: Vault[] = resp.map(item => ({ id: item.id || "", name: item.name || "" }));
			const updatedVaults = [defaultVault, ...folderVaults];
			
			// Update the vaults store with properly typed data
			vaults.update(() => updatedVaults as any);

			unsubscribeResourceUpdate = await listen(
				"merge-update",
				async (event) => {
					console.log(event);
					const payload = event.payload as MergeUpdatePayload;
					let mergedDocument = mergeDocuments(
						payload.local_resource,
						payload.remote_resource,
					) as MergedDocument;
					
					emit("merge-complete", {
						mergedDocument,
						deviceId: payload.device_id,
						userId: payload.user_id,
						vectorClock: payload.vector_clock,
						resourceId: mergedDocument.resource_id,
					});
					console.log(mergedDocument);
				},
			);
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
