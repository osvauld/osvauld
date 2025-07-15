<script lang="ts">
	import LL from "@osvauld/password-manager-common/i18n/i18n-svelte";
	import Profile from "@osvauld/password-manager-common/icons/profile.svelte";
	import Add from "@osvauld/password-manager-common/icons/mobileAdd.svelte";
	import PwdGen from "@osvauld/password-manager-common/icons/pwdGen.svelte";
	import Home from "@osvauld/password-manager-common/icons/mobileHome.svelte";
	import Star from "@osvauld/password-manager-common/icons/star.svelte";
	import {
		categorySelection,
		currentVault,
		currentLayout,
		favoriteCredentials,
		credentialListWithType,
		credentialLayoutType,
	} from "../../store/mobile.ui.store";
	import MultipleUsers from "@osvauld/password-manager-common/icons/multipleUsers.svelte";

	$: isFavouriteSelected = $credentialListWithType === "favourites";
	$: isAddSelected = $currentLayout === "category";
	let isListSelected = false;

	const handleAddCredential = (route) => {
		currentLayout.set(route);
		if (route === "credential ") categorySelection.set(!$categorySelection);
		else if (route === "home") {
			currentVault.set({ id: "all", name: "All" });
			categorySelection.set(false);
		}
	};

	const handleFavouriteSelection = () => {
		$favoriteCredentials
			? credentialListWithType.set("")
			: credentialListWithType.set("favourites");
		favoriteCredentials.set(!$favoriteCredentials);
	};

	const handleStateReset = () => {
		currentLayout.set("home");
		credentialListWithType.set("");
		credentialLayoutType.set("addition");
	};
</script>

<nav
	class="h-[68px] py-2 w-full fixed bottom-0 bg-mobile-navBlue flex text-base font-sans font-normal text-mobile-iconPrimary">
	<button
		class=" flex-1 flex justify-center items-center flex-col"
		on:click|stopPropagation={handleStateReset}
		><span class="flex justify-center items-center"
			><Home color={isListSelected ? "#89B4FA" : "#5B5D6D"} /></span>
		<span class:text-osvauld-carolinablue={isListSelected}>Home</span></button>
	<!-- 
	<button
		class=" flex-1 flex justify-center items-center flex-col"
		on:click="{handleVaultManger}">
		<span class="flex justify-center items-center"
			><MultipleUsers color="{isHome ? '#89B4FA' : '#5B5D6D'}" /></span>
		<span class="capitalize" class:text-mobile-highlightBlue="{isHome}"
			>{$currentVault?.id === "all"
				? "All Vaults"
				: `${$currentVault?.name}`}</span>
	</button> -->
	<button
		class=" flex-1 flex justify-center items-center flex-col"
		on:click={() => handleAddCredential("category")}>
		<span class="flex justify-center items-center"
			><Add color={isAddSelected ? "#89B4FA" : "#5B5D6D"} /></span>
		<span class:text-osvauld-carolinablue={isAddSelected}>Add</span></button>
	<!-- <button class=" flex-1 flex justify-center items-center flex-col"
		><span class="flex justify-center items-center"
			><PwdGen color="{'#5B5D6D'}" /></span>
		<span>Generator</span></button> -->
	<!-- <button
		on:click="{() => handleClick('profile')}"
		class=" flex-1 flex flex-col justify-center items-center">
		<span class="flex justify-center items-center"
			><Profile color="{'#5B5D6D'}" /></span>
		<span>{$LL.tabs.profile()}</span></button> -->
	<!-- <button class=" flex-1 flex justify-center items-center flex-col"
		><span class="flex justify-center items-center"
			><MultipleUsers color="#5B5D6D" /></span>
		<span>Switch</span></button> -->
	<button
		class=" flex-1 flex justify-center items-center flex-col"
		on:click|stopPropagation={handleFavouriteSelection}
		><span class="flex justify-center items-center"
			><Star color={isFavouriteSelected ? "#89B4FA" : "#5B5D6D"} /></span>
		<span class:text-osvauld-carolinablue={isFavouriteSelected}>Favourites</span
		></button>
</nav>
