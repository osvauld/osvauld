<script lang="ts">
	import ContentWrapper from "../sections/ContentWrapper.svelte";
	import HeaderSection from "../sections/HeaderSection.svelte";
	import NavigationPanel from "../sections/NavigationPanel.svelte";
	import { onMount } from "svelte";
	import { vaults } from "../store/desktop.ui.store";
	import { sendMessage } from "../../../lib/components/dashboard/helper";

	onMount(async () => {
		try {
			const resp = await sendMessage("getFolder");
			const updatedVaults = [{ id: "all", name: "All Vaults" }, ...resp];
			vaults.set(updatedVaults);
		} catch (e) {
			console.log("Error received ===>", e);
		}
	});
</script>

<div class="w-full h-full bg-osvauld-ninjablack flex flex-col overflow-hidden">
	<HeaderSection />
	<div class="grow flex overflow-hidden">
		<NavigationPanel />
		<ContentWrapper />
	</div>
</div>
