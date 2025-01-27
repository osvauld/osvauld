<script lang="ts">
	import LL from "../../../i18n/i18n-svelte";
	import Profile from "../../../icons/profile.svelte";
	import Add from "../../../icons/mobileAdd.svelte";
	import PwdGen from "../../../icons/pwdGen.svelte";
	import Home from "../../../icons/mobileHome.svelte";
	import Star from "../../../icons/star.svelte";
	import {
		categorySelection,
		currentVault,
		currentLayout,
		bottomNavActive,
		vaultSwitchActive,
	} from "../../store/mobile.ui.store";
	import MultipleUsers from "../../../icons/multipleUsers.svelte";

	const handleClick = (route) => {
		currentLayout.set(route);
		if (route === "credential ") categorySelection.set(!$categorySelection);
		else if (route === "home") {
			currentVault.set({ id: "all", name: "All" });
			categorySelection.set(false);
		}
	};

	const handleVaultManger = () => {
		if ($vaultSwitchActive) {
			bottomNavActive.set(true);
		}

		vaultSwitchActive.set(!$vaultSwitchActive);
	};
</script>

<nav
	class="h-[68px] py-2 w-full fixed bottom-0 bg-mobile-navBlue flex text-base font-sans font-normal text-mobile-iconPrimary">
	<button
		class=" flex-1 flex justify-center items-center flex-col"
		on:click="{handleVaultManger}">
		<span class="flex justify-center items-center"
			><Home color="#5B5D6D" /></span>
		<span class="capitalize"
			>{$currentVault?.id === "all"
				? "All Vaults"
				: `${$currentVault?.name}`}</span>
	</button>
	<button
		class=" flex-1 flex justify-center items-center flex-col"
		on:click="{() => handleClick('category')}">
		<span class="flex justify-center items-center"
			><Add color="#5B5D6D" /></span>
		<span>{$LL.tabs.add()}</span></button>
	<!-- <button
		class=" flex-1 flex justify-center items-center flex-col"
		on:click="{() => handleClick('generator')}"
		><span class="flex justify-center items-center"
			><PwdGen color="{'#5B5D6D'}" /></span>
		<span>{$LL.tabs.generator()}</span></button> -->
	<!-- <button
		on:click="{() => handleClick('profile')}"
		class=" flex-1 flex flex-col justify-center items-center">
		<span class="flex justify-center items-center"
			><Profile color="{'#5B5D6D'}" /></span>
		<span>{$LL.tabs.profile()}</span></button> -->
	<button class=" flex-1 flex justify-center items-center flex-col"
		><span class="flex justify-center items-center"
			><MultipleUsers color="#5B5D6D" /></span>
		<span>Shared</span></button>
	<button class=" flex-1 flex justify-center items-center flex-col"
		><span class="flex justify-center items-center"
			><Star color="#5B5D6D" /></span>
		<span>Favourites</span></button>
</nav>
