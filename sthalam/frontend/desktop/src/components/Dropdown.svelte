<script lang="ts">
	import { onMount, onDestroy } from "svelte";

	interface DropdownOption {
		value: string;
		label: string;
		count?: number;
		disabled?: boolean;
	}

	interface Props {
		value: string | null;
		options: DropdownOption[];
		onChange: (value: string | null) => void;
		placeholder?: string;
		class?: string;
		disabled?: boolean;
	}

	let {
		value = $bindable(),
		options,
		onChange,
		placeholder = "Select an option",
		class: className = "",
		disabled = false
	}: Props = $props();

	let isOpen = $state(false);
	let dropdownRef: HTMLDivElement | null = null;
	let buttonRef: HTMLButtonElement | null = null;

	// Find the selected option's label
	const selectedLabel = $derived(() => {
		const selected = options.find(opt => opt.value === value);
		if (selected) {
			return selected.count !== undefined
				? `${selected.label} (${selected.count})`
				: selected.label;
		}
		return placeholder;
	});

	function toggleDropdown() {
		if (!disabled) {
			isOpen = !isOpen;
		}
	}

	function selectOption(optionValue: string | null, optionDisabled?: boolean) {
		if (optionDisabled) return;

		value = optionValue;
		onChange(optionValue);
		isOpen = false;
	}

	function handleClickOutside(event: MouseEvent) {
		if (dropdownRef && !dropdownRef.contains(event.target as Node)) {
			isOpen = false;
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === "Escape" && isOpen) {
			isOpen = false;
			buttonRef?.focus();
		}
	}

	onMount(() => {
		document.addEventListener("click", handleClickOutside);
		document.addEventListener("keydown", handleKeydown);
	});

	onDestroy(() => {
		document.removeEventListener("click", handleClickOutside);
		document.removeEventListener("keydown", handleKeydown);
	});
</script>

<div class="dropdown-container {className}" bind:this={dropdownRef}>
	<button
		bind:this={buttonRef}
		class="dropdown-button"
		class:disabled={disabled}
		onclick={toggleDropdown}
		disabled={disabled}
		type="button"
		aria-haspopup="listbox"
		aria-expanded={isOpen}
	>
		<span class="dropdown-label" class:placeholder={!value}>
			{selectedLabel()}
		</span>
		<svg
			class="dropdown-arrow"
			class:open={isOpen}
			fill="currentColor"
			viewBox="0 0 20 20"
		>
			<path
				fill-rule="evenodd"
				d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z"
				clip-rule="evenodd"
			/>
		</svg>
	</button>

	{#if isOpen}
		<div class="dropdown-menu" role="listbox">
			{#each options as option (option.value)}
				<button
					class="dropdown-option"
					class:selected={value === option.value}
					class:disabled={option.disabled}
					onclick={() => selectOption(option.value, option.disabled)}
					disabled={option.disabled}
					type="button"
					role="option"
					aria-selected={value === option.value}
				>
					<span class="option-label">
						{option.label}
						{#if option.count !== undefined}
							<span class="option-count">({option.count})</span>
						{/if}
					</span>
					{#if value === option.value}
						<svg class="option-check" fill="currentColor" viewBox="0 0 20 20">
							<path
								fill-rule="evenodd"
								d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
								clip-rule="evenodd"
							/>
						</svg>
					{/if}
				</button>
			{/each}
		</div>
	{/if}
</div>

<style>
	.dropdown-container {
		position: relative;
		width: 100%;
	}

	.dropdown-button {
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding: 0.5rem 0.75rem;
		background: var(--color-osvauld-fieldActive);
		color: var(--color-osvauld-quarzowhite);
		border: 1px solid var(--color-osvauld-borderColor);
		border-radius: 6px;
		font-size: 0.875rem;
		cursor: pointer;
		transition: all 0.2s;
		text-align: left;
	}

	.dropdown-button:hover:not(.disabled) {
		border-color: var(--color-osvauld-activeBorder);
		background: var(--color-osvauld-modalFieldActive);
	}

	.dropdown-button:focus:not(.disabled) {
		outline: none;
		border-color: var(--color-livnotePink);
		box-shadow: 0 0 0 1px var(--color-livnotePink);
	}

	.dropdown-button.disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.dropdown-label {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.dropdown-label.placeholder {
		color: var(--color-osvauld-textPassive);
	}

	.dropdown-arrow {
		width: 1.25rem;
		height: 1.25rem;
		color: var(--color-osvauld-textPassive);
		transition: transform 0.2s;
		flex-shrink: 0;
	}

	.dropdown-arrow.open {
		transform: rotate(180deg);
	}

	.dropdown-menu {
		position: absolute;
		top: calc(100% + 0.25rem);
		left: 0;
		right: 0;
		max-height: 16rem;
		overflow-y: auto;
		background: var(--color-osvauld-frameblack);
		border: 1px solid var(--color-osvauld-activeBorder);
		border-radius: 6px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
		z-index: 50;
		padding: 0.25rem;
	}

	.dropdown-menu::-webkit-scrollbar {
		width: 4px;
	}

	.dropdown-menu::-webkit-scrollbar-track {
		background: transparent;
	}

	.dropdown-menu::-webkit-scrollbar-thumb {
		background-color: var(--color-osvauld-iconblack);
		border-radius: 4px;
	}

	.dropdown-option {
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding: 0.5rem 0.75rem;
		background: transparent;
		color: var(--color-osvauld-quarzowhite);
		border: none;
		border-radius: 4px;
		font-size: 0.875rem;
		cursor: pointer;
		transition: all 0.15s;
		text-align: left;
	}

	.dropdown-option:hover:not(.disabled) {
		background: var(--color-osvauld-fieldActive);
	}

	.dropdown-option.selected {
		background: var(--color-osvauld-fieldActive);
		color: var(--color-livnotePink);
	}

	.dropdown-option.disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.option-label {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.option-count {
		color: var(--color-osvauld-textPassive);
		font-size: 0.8125rem;
	}

	.option-check {
		width: 1rem;
		height: 1rem;
		flex-shrink: 0;
		color: var(--color-livnotePink);
	}
</style>
