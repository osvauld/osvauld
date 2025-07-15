<script lang="ts">
	import AddCredential from "../views/AddCredentialMobile.svelte";
	import {
		currentLayout,
		credentialLayoutType,
	} from "../../store/mobile.ui.store";
	import ViewCredential from "../views/ViewCredentialMobile.svelte";
</script>

<div
	class="w-full h-full bg-mobile-bgPrimary flex flex-col overflow-hidden p-3">
	{#if $credentialLayoutType === "addition"}
		<AddCredential />
	{:else if $credentialLayoutType === "view"}
		<ViewCredential />
	{/if}
</div>
