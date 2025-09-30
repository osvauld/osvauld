<script lang="ts">
	import { MobileNote, FavStar } from "../../icons";

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

<div
	class="pl-0.5 my-1"
	role="treeitem"
	aria-selected={isSelected}
	tabindex="0"
>
	<button
		class="w-full flex items-center gap-3 p-2.5 rounded-lg transition-colors duration-150 focus:outline-none
			{isActiveState
			? 'text-osvauld-sideListTextActive bg-osvauld-fieldActive'
			: 'text-osvauld-fieldText hover:text-osvauld-sideListTextActive hover:bg-osvauld-fieldActive'}"
		onclick={onSelect}
		onkeydown={handleKeyDown}
		onmouseenter={handleMouseEnter}
		onmouseleave={handleMouseLeave}
		aria-label="Select note: {note.title || 'Untitled Note'}"
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
		<span class="flex-1 truncate text-left text-sm font-light">
			{note.title || "Untitled Note"}
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
