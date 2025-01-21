<script>
	import { invoke } from "@tauri-apps/api/core";
	import { setLocale } from "../../i18n/i18n-svelte";
	import { loadLocaleAsync } from "../../i18n/i18n-util.async";
	import { onMount } from "svelte";

	import DefaultLayout from "./layouts/DefaultLayout.svelte";
	import CredentialLayout from "./layouts/CredentialLayout.svelte";
	import CategoryLayout from "./layouts/CategoryLayout.svelte";
	import ProfileLayout from "./layouts/ProfileLayout.svelte";
	import BottomNavigation from "./sections/BottomNavigation.svelte";
	import {
		selectedCredentialType,
		categorySelection,
		currentLayout,
		bottomNavActive,
	} from "../store/mobile.ui.store";

	const SUPPORTED_LANGUAGES = [
		"en",
		"de",
		"ar",
		"fr",
		"hi",
		"it",
		"ja",
		"ko",
		"nl",
		"pl",
		"ru",
		"tr",
		"zh",
	];

	// $: currentLayout = $selectedCredentialType
	// 	? "credential"
	// 	: $categorySelection
	// 		? "category"
	// 		: "default";

	async function initializeLanguage() {
		try {
			const locale = await invoke("get_system_locale");
			const deviceLanguage = String(locale).split(/[-_]/)[0].toLowerCase();
			const languageToUse = SUPPORTED_LANGUAGES.includes(deviceLanguage)
				? deviceLanguage
				: "en";
			// const languageToUse = "fr";
			await loadLocaleAsync(languageToUse);
			setLocale(languageToUse);
		} catch (error) {
			console.error("Error detecting system language:", error);
			await loadLocaleAsync("en");
			setLocale("en");
		}
	}

	onMount(() => {
		initializeLanguage();
	});
</script>

{#if $currentLayout === "credential"}
	<CredentialLayout />
{:else if $currentLayout === "category"}
	<CategoryLayout />
{:else if $currentLayout === "profile"}
	<ProfileLayout />
{:else if $currentLayout === "home"}
	<DefaultLayout />
{/if}

{#if $bottomNavActive}
	<BottomNavigation />
{/if}
