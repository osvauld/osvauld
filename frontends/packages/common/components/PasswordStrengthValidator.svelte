<script lang="ts">
	// Using $props for component properties
	let { passphrase = "", onStrengthChange = (isAcceptable: boolean) => {} } =
		$props();

	// Define password conditions
	const conditions = [
		{ key: "lowercase", label: "Lowercase letter", regex: /[a-z]/ },
		{ key: "uppercase", label: "Uppercase letter", regex: /[A-Z]/ },
		{ key: "number", label: "Number", regex: /[0-9]/ },
		{
			key: "specialChar",
			label: "Special character",
			regex: /[!@#$%^&*()_+\-=\[\]{};':"\\|,.<>/?]/,
		},
		{ key: "length", label: "At least 8 characters" },
	];

	// Calculate strength results based on current passphrase
	const strengthResults = $derived(
		conditions.map((condition) => ({
			...condition,
			met:
				condition.key === "length"
					? passphrase.length >= 8
					: (condition.regex?.test(passphrase) ?? false),
		})),
	);

	// Calculate score based on how many conditions are met
	const strengthScore = $derived(
		strengthResults.filter((result) => result.met).length,
	);

	// Calculate if passphrase is acceptable
	const isAcceptable = $derived(strengthScore > 4);

	// Notify parent component when strength changes
	$effect(() => {
		onStrengthChange(isAcceptable);
	});

	function getStrengthColor(score: number): string {
		if (score <= 2) return "bg-red-500";
		if (score <= 4) return "bg-yellow-500";
		return "bg-green-500";
	}

	function getStrengthWidth(score: number): string {
		return `${(score / conditions.length) * 100}%`;
	}
</script>

<div class="w-[300px] rounded-xl shadow-md overflow-hidden p-2">
	<div class="mb-4">
		<div class="h-2 w-full bg-gray-300 rounded-full">
			{#if strengthScore > 0}
				<div
					class="h-full rounded-full transition-all duration-300 ease-out {getStrengthColor(
						strengthScore,
					)}"
					style="width: {getStrengthWidth(strengthScore)}">
				</div>
			{/if}
		</div>
		<p
			class="text-xs mt-1 font-light text-osvauld-sheffieldgrey text-center tracking-wide">
			Passphrase should include at least
			{#each strengthResults as condition, index}
				<span class={condition.met ? "text-green-500" : "text-yellow-300"}>
					{condition.label}{index < strengthResults.length - 1 ? "," : ""}
				</span>
				{#if index < strengthResults.length - 1}&nbsp;{/if}
			{/each}
		</p>
	</div>
</div>
