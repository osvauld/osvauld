<script lang="ts">
	import { createEventDispatcher } from "svelte";
	import { fetchSensitiveFieldsByCredentialId } from "../apis";
	import EncryptedField from "./EncryptedField.svelte";
	import PlainField from "./PlainField.svelte";
	import { More } from "../icons";
	import {
		showCredentialDetailsDrawer,
		searchedCredential,
		showMoreOptions,
		buttonRef,
		modalManager,
	} from "../store";
	import { Credential, Fields } from "../dtos";
	import { tweened } from "svelte/motion";
	const dispatch = createEventDispatcher();
	export let credential: Credential;
	export let checked = false;
	export let privateFolder;
	let sensitiveFields: Fields[] = [];
	let decrypted = false;
	let hoverEffect = false;
	let hoverTimeout: any;
	let borderHighLight = tweened(0, { duration: 700 });
	$: {
		if ($searchedCredential?.credentialId === credential.credentialId) {
			borderHighLight.set(1);
			setTimeout(() => {
				borderHighLight.set(0);
				searchedCredential.set(null);
			}, 500);
		}
	}
	function toggleCheck() {
		checked = !checked;
		dispatch("check", checked);
	}
	function handleMouseEnter() {
		hoverEffect = true;
		if (!decrypted) {
			hoverTimeout = setTimeout(async () => {
				const response = await fetchSensitiveFieldsByCredentialId(
					credential.credentialId,
				);
				sensitiveFields = response.data;
			}, 300);
		}
	}
	function handleMouseLeave() {
		if (!$showCredentialDetailsDrawer) {
			sensitiveFields = [];
		}
		clearTimeout(hoverTimeout);
		decrypted = false;
		hoverEffect = false;
	}

	const triggerMoreActions = (e: any) => {
		buttonRef.set(e.currentTarget);
		modalManager.set({
			id: credential.credentialId,
			name: credential.name,
			type: "Credential",
		});
		dispatch("action", true);
		showMoreOptions.set(true);
	};

	const handleClick = async () => {
		if (sensitiveFields.length) {
			clearTimeout(hoverTimeout);
			const response = await fetchSensitiveFieldsByCredentialId(
				credential.credentialId,
			);
			sensitiveFields = response.data;
		}
		dispatch("select", sensitiveFields);
		sensitiveFields = [];
	}; /* eslint-disable */
</script>

<style>
	input[type="checkbox"] {
		appearance: none;
		position: relative;
	}

	input[type="checkbox"]::after {
		content: "";
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: 100%;
		height: 100%;
		background-color: #b4befe;
		z-index: 1;
		opacity: 0;
	}

	input[type="checkbox"]::before {
		content: "";
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: 15px;
		height: 11px;
		background-color: #0d1117;
		clip-path: path(
			"M4.60852 10.4792C4.34185 10.4792 4.07518 10.3792 3.87102 10.175L0.000183105 6.3L0.883517 5.41667L4.60852 9.14167L13.7502 0L14.6335 0.883333L5.34602 10.1708C5.14185 10.375 4.87518 10.475 4.60852 10.475V10.4792Z"
		);
		opacity: 0;
		z-index: 10;
		transition: opacity 0.2s ease-in-out;
	}

	input[type="checkbox"]:checked::before {
		opacity: 1;
	}

	input[type="checkbox"]:checked::after {
		opacity: 1;
	}
</style>

<button
	class="mb-1 overflow-x-hidden flex-none rounded-xl text-osvauld-chalkwhite border border-osvauld-iconblack {checked &&
		'shadow-[0_0_0_1px_#B4BEFE]'}"
	style="border: {$borderHighLight ? '1px solid #89B4FA' : ''}"
	on:mouseenter="{handleMouseEnter}"
	on:mouseleave="{handleMouseLeave}"
	on:click="{handleClick}"
>
	<button
		class="container mx-auto py-3 pl-3 pr-1 relative group bg-osvauld-cardshade rounded-xl"
	>
		<div
			class="flex {credential.accessType !== 'manager' && !privateFolder
				? 'justify-start'
				: 'justify-center'} items-center border-osvauld-iconblack pb-2"
			on:click|stopPropagation
		>
			{#if credential.accessType === "manager" && !privateFolder}
				<input
					type="checkbox"
					id="{credential.credentialId}"
					class="bg-osvauld-cardshade mr-2
        {hoverEffect
						? 'border-osvauld-placeholderblack'
						: 'border-osvauld-darkLineSeperator'} checked:bg-osvauld-activelavender focus:text-osvauld-activelavender hover:text-osvauld-activelavender active:outline-none focus:ring-offset-0 focus:ring-0 cursor-pointer"
					on:change|stopPropagation="{(e) => {
						toggleCheck();
					}}"
					{checked}
				/>
			{/if}
			<label
				class="text-lg font-light text-left ml-2 cursor-pointer w-[10rem] overflow-x-hidden whitespace-nowrap {hoverEffect
					? 'text-osvauld-sideListTextActive'
					: 'text-osvauld-fieldTextActive '} "
				for="{credential.credentialId}"
			>
				{credential.name}
			</label>
			{#if credential.accessType === "manager"}
				<button class="pr-2" on:click="{triggerMoreActions}">
					<More />
				</button>
			{/if}
		</div>
		<div
			class="border-b border-osvauld-iconblack w-[calc(100%+1.5rem)] -translate-x-3"
		></div>
		<div
			class="w-[15rem] {credential.description.length !== 0
				? 'h-[11.5rem]'
				: 'h-[15rem]'} overflow-y-scroll scrollbar-thin pr-0 {hoverEffect
				? 'active'
				: ''} mt-2"
		>
			{#each credential.fields as field}
				{#if field.fieldName !== "Domain"}
					<PlainField
						bgColor="{null}"
						fieldName="{field.fieldName}"
						fieldValue="{field.fieldValue}"
						{hoverEffect}
					/>
				{/if}
			{/each}
			{#if sensitiveFields}
				{#each sensitiveFields as field}
					<EncryptedField
						bgColor="{null}"
						fieldName="{field.fieldName}"
						fieldValue="{field.fieldValue}"
						fieldType="{field.fieldType}"
						{hoverEffect}
					/>
				{/each}
			{/if}
		</div>
		<div
			class="{credential.description.length !== 0 ? 'visible' : 'invisible'}"
		>
			<label
				class="text-osvauld-dusklabel block text-left text-xs font-normal"
				for="credential-description"
			>
				Description
			</label>
			<div
				class="mt-1 w-[14.3rem] {credential.description.length !== 0
					? 'h-[4rem]'
					: ''} py-1 px-2 overflow-y-scroll rounded-lg text-left scrollbar-thin resize-none text-sm
    {hoverEffect
					? 'text-osvauld-fieldTextActive bg-osvauld-fieldActive'
					: 'text-osvauld-fieldText'}"
				id="credential-description"
			>
				{credential.description}
			</div>
		</div>
	</button>
</button>
