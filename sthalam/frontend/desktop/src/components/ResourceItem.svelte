<script lang="ts">
	import type { Resource } from "../types";
	import FileText from "../lib/icons/src/icons/fileText.svelte";

	interface Props {
		resource: Resource;
		isSelected: boolean;
		onSelect: () => void;
	}

	let { resource, isSelected, onSelect }: Props = $props();

	let isHovered = $state(false);

	const isActive = $derived(isSelected || isHovered);
</script>

<div class="pl-1.5 my-1">
	<button
		class="w-full flex items-center gap-3 p-2.5 rounded-lg transition-colors duration-150 min-w-0"
		class:text-osvauld-sideListTextActive={isActive}
		class:bg-osvauld-fieldActive={isActive}
		class:text-textActive={!isActive}
		class:hover:text-osvauld-sideListTextActive={!isActive}
		class:hover:bg-osvauld-fieldActive={!isActive}
		onclick={onSelect}
		onmouseenter={() => (isHovered = true)}
		onmouseleave={() => (isHovered = false)}
		title={resource.title || "Untitled"}
	>
		<!-- Page icon -->
		<span class="shrink-0">
			<FileText color={isActive ? "#F2F2F0" : "#85889C"} width={20} height={20} />
		</span>

		<!-- Page title -->
		<span
			class="flex-1 text-left text-sm font-normal min-w-0 overflow-hidden text-ellipsis whitespace-nowrap"
		>
			{resource.title || "Untitled"}
		</span>
	</button>
</div>
