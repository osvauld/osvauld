<script lang="ts">
	import { onMount } from "svelte";
	import BinIcon from "../../../icons/binIcon.svelte";
	import { sendMessage } from "../../../lib/components/dashboard/helper";
	import {
		currentVault,
		deleteConfirmationModal,
	} from "../../store/mobile.ui.store";
</script>

<div
	class="fixed top-0 w-full flex justify-between items-center bg-mobile-bgSeconary text-mobile-textPrimary text-lg font-semibold px-4 py-2">
	<span class="max-w-3/4 truncate capitalize">{$currentVault.name}</span>
	<button><BinIcon /></button>
</div>
