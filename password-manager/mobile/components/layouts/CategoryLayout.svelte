<script lang="ts">
	import CategoriesToAdd from "../views/CategoriesToAdd.svelte";
	import MobileLeftArrow from "@osvauld/password-manager-common/icons/mobileLeftArrow.svelte";
	import { currentLayout } from "../../store/mobile.ui.store";

	const goBack = () => {
		currentLayout.set("home");
	};
</script>

<div
	class="w-screen h-screen bg-mobile-bgPrimary flex flex-col relative pb-[60px] overflow-hidden">
	<nav class="w-full h-[48px] px-3 flex items-center gap-2 flex-shrink-0">
		<button on:click="{goBack}" class="p-2.5 rounded-lg bg-mobile-bgSeconary">
			<MobileLeftArrow />
		</button>
	</nav>
	<CategoriesToAdd />
</div>
