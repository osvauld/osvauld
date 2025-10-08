<script lang="ts">
	import { onMount } from "svelte";

	let isDark = $state(false);

	onMount(() => {
		// Load theme from localStorage
		const saved = localStorage.getItem("theme");
		isDark = saved === "dark";
		applyTheme();
	});

	function toggleTheme() {
		isDark = !isDark;
		localStorage.setItem("theme", isDark ? "dark" : "light");
		applyTheme();
	}

	function applyTheme() {
		if (isDark) {
			document.documentElement.setAttribute("data-theme", "dark");
		} else {
			document.documentElement.removeAttribute("data-theme");
		}
	}
</script>

<button class="theme-toggle" onclick={toggleTheme} title="Toggle theme">
	{#if isDark}
		<span class="icon">☀️</span>
		<span class="label">Light</span>
	{:else}
		<span class="icon">🌙</span>
		<span class="label">Dark</span>
	{/if}
</button>

<style>
	.theme-toggle {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 1rem;
		border: 1px solid var(--border-color, #e0e0e0);
		border-radius: 6px;
		background: var(--bg-primary, white);
		color: var(--text-primary, #333);
		cursor: pointer;
		transition: all 0.2s;
		font-size: 0.875rem;
		font-weight: 500;
	}

	.theme-toggle:hover {
		background: var(--bg-hover, #f5f5f5);
		border-color: #667eea;
	}

	.icon {
		font-size: 1.25rem;
	}
</style>
