<script lang="ts">
	import { MobileNote, FavStar } from "@osvauld/icons";

	interface Props {
		note: any;
		isSelected: boolean;
		onSelect: () => void;
		onHover: (noteId: string) => void;
		onHoverLeave: () => void;
		isHovered: boolean;
	}

	let { note, isSelected, onSelect, onHover, onHoverLeave, isHovered }: Props =
		$props();

	function handleKeyDown(event: KeyboardEvent) {
		if (event.key === "Enter" || event.key === " ") {
			event.preventDefault();
			onSelect();
		}
	}

	function handleMouseEnter() {
		onHover(note.id);
	}

	function handleMouseLeave() {
		onHoverLeave();
	}

	const isActiveState = $derived(isSelected || isHovered);
</script>

<div class="pl-1.5 my-1">
	<button
		class="w-full flex items-center gap-3 p-2.5 rounded-lg transition-colors duration-150 min-w-0
			{isActiveState
			? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
			: 'text-textActive hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive'}"
		role="treeitem"
		aria-selected={isSelected}
		onclick={onSelect}
		onkeydown={handleKeyDown}
		onmouseenter={handleMouseEnter}
		onmouseleave={handleMouseLeave}
		aria-label="Select note: {note.title || 'Untitled'}"
	>
		<!-- Note icon -->
		<span class="shrink-0">
			<MobileNote
				color={isActiveState ? "#F2F2F0" : "#85889C"}
				width={20}
				height={20}
			/>
		</span>

		<!-- Note title -->
		<span
			class="flex-1 text-left text-sm font-normal min-w-0 overflow-hidden text-ellipsis whitespace-nowrap"
			title={note.title || "Untitled"}
		>
			{note.title || "Untitled"}
		</span>

		<!-- Note metadata (optional - could show last modified, etc.) -->
		{#if note.favourite}
			<span
				class="shrink-0 text-xs"
				aria-label="Favorited note"
				title="Favorited"
			>
				<FavStar size={16} />
			</span>
		{/if}
	</button>
</div>
