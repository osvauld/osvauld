<script lang="ts">
	import { setbackground } from "../helper";
	import BinIcon from "../../basic/icons/binIcon.svelte";
	import DownArrow from "../../basic/icons/downArrow.svelte";
	import { createEventDispatcher } from "svelte";
	import AccessSelector from "./AccessSelector.svelte";
	import InfoIcon from "../../basic/icons/infoIcon.svelte";

	const dispatch = createEventDispatcher();
	export let item;
	export let index;
	export let isUser;
	export let editPermissionTrigger;
	export let reverseModal;
	let permissionChanged = false;
	let accessSelectorIdentifier = null;
	let showTooltip = false;

	$: if (!editPermissionTrigger) accessSelectorIdentifier = null;

	const handleItemRemove = () => {
		dispatch("remove");
	};

	const eventPasser = (e) => {
		item = { ...item, accessType: e.detail.permission };
		permissionChanged = !permissionChanged;
		dispatch("permissonChange", e.detail.permission);
		accessSelectorIdentifier = null;
	};
	const changePermissionHandler = () => {
		if (editPermissionTrigger) {
			if (
				(item.accessSource && item.accessSource === "acquired") ||
				!item.accessSource
			) {
				permissionChanged = !permissionChanged;
				accessSelectorIdentifier = index;
			}
		}
	};
</script>

<style>
	.tooltip {
		position: absolute;
		top: -30px;
		right: 0;
		border: 1px solid #2f303e;
		background-color: #0d0e13;
		color: #6e7681;
		padding: 6px 10px;
		border-radius: 6px;
		font-size: 12px;
		white-space: nowrap;
		z-index: 10;
	}
</style>

<div
	class="relative w-full my-2 pl-1 pr-0.5 border border-osvauld-iconblack rounded-lg cursor-pointer flex items-center justify-between text-osvauld-sheffieldgrey font-normal text-base"
>
	<div class="flex items-center justify-start">
		<p class="p-1 w-3/4 whitespace-nowrap">
			{item.name}
		</p>
	</div>
	{#if (item.accessSource && item.accessSource === "acquired") || !item.accessSource}
		{#if accessSelectorIdentifier === index}
			<AccessSelector
				{reverseModal}
				on:select="{(e) => eventPasser(e)}"
				on:closeSelector="{() => (accessSelectorIdentifier = null)}"
			/>
		{/if}
		{#if editPermissionTrigger}
			<button class="ml-auto" on:click="{handleItemRemove}">
				{#if item.accessSource === "acquired" || !isUser}
					<BinIcon />
				{/if}
			</button>
		{/if}
	{:else if item.accessSource && item.accessSource === "inherited"}
		{#if editPermissionTrigger}
			<span
				class="ml-auto relative"
				on:mouseenter="{() => (showTooltip = true)}"
				on:mouseleave="{() => (showTooltip = false)}"
				role="button"
				tabindex="-1"
			>
				<InfoIcon />
				{#if showTooltip}
					<div class="tooltip">Inherited from folder</div>
				{/if}
			</span>
		{/if}
	{/if}
	<div class="flex justify-center items-center ml-2">
		<button
			class="w-[9.8rem] rounded-md cursor-pointer px-1 py-0.5 flex justify-around items-center {setbackground(
				item.accessType,
			)}"
			on:click="{changePermissionHandler}"
		>
			<span>{item.accessType === "manager" ? "Manager" : "Reader"}</span>
			{#if editPermissionTrigger && ((item.accessSource && item.accessSource === "acquired") || !item.accessSource)}
				<DownArrow type="{item.accessType}" />
			{/if}
		</button>
	</div>
</div>
