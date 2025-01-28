<script lang="ts">
	import Lens from "../../../icons/lens.svelte";
	import Options from "../../../icons/options.svelte";
	import DownArrow from "../../../icons/mobileDownArrow.svelte";
	import LL from "../../../i18n/i18n-svelte";
	import { currentVault, currentLayout } from "../../store/mobile.ui.store";
	import Profile from "../../../icons/profile.svelte";

	const handleProfileSelection = () => {
		currentLayout.set("profile");
	};
</script>

<div class="px-4 py-1 mb-4">
	<div
		class="mt-10 h-[48px] flex justify-start items-center bg-mobile-bgSeconary rounded-lg px-3 py-2.5">
		<span><Options color="#4D4F60" /></span>
		<span
			class="ml-2 text-base text-mobile-textSecondary font-normal flex items-center">
			{$currentVault.id === "all"
				? "Search All Vaults"
				: `Search ${$currentVault.name}`}
		</span>
		<button class="ml-auto" on:click="{handleProfileSelection}"
			><Profile color="#4D4F60" /></button>
	</div>
</div>
