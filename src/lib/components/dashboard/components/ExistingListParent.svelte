<script lang="ts">
	import { createEventDispatcher } from "svelte";
	import ExistingListItem from "./ExistingListItem.svelte";

	export let existingItemsData;
	export let editPermissionTrigger;
	export let isUser;

	const dispatch = createEventDispatcher();

	const handleRemoval = (item) => {
		dispatch("remove", item);
	};

	const handlePermissionChange = (e, item) => {
		dispatch("permissionChange", { item, permission: e.detail });
	};
</script>

<div class="py-2 px-0 mt-0 mb-2 w-full rounded-lg min-h-0 max-h-[50vh]">
	<div
		class="overflow-y-scroll scrollbar-thin h-[16rem] pr-1 bg-osvauld-frameblack w-full"
	>
		{#if existingItemsData}
			{#each existingItemsData as item, index}
				<!-- TODO: user should not be able to remove themselves -->
				<ExistingListItem
					{index}
					{item}
					{isUser}
					{editPermissionTrigger}
					reverseModal="{existingItemsData.length > 3 &&
						index > existingItemsData.length - 3}"
					on:remove="{() => handleRemoval(item)}"
					on:permissonChange="{(e) => handlePermissionChange(e, item)}"
				/>
			{/each}
		{/if}
	</div>
</div>
