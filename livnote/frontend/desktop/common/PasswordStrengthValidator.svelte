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
		{ key: "length", label: "At least 6 characters" },
	];

	// Calculate strength results based on current passphrase
	const strengthResults = $derived(
		conditions.map((condition) => ({
			...condition,
			met:
				condition.key === "length"
					? passphrase.length >= 6
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
		if (score > 0 && score <= 2) return "#ef4444"; // Red for score 1-2
		if (score <= 4) return "#f97316"; // Orange for score 3-4
		return "#22c55e"; // Green for score 5
	}
</script>

<div class="w-[24rem] rounded-xl shadow-md overflow-hidden p-2">
	<div class="mb-4">
		<!-- Segmented strength bar that fills based on score -->
		<div class="flex justify-between items-center w-full mb-2">
			{#each strengthResults as condition, index}
				<div class="h-1 flex-1 mx-1 rounded-full bg-[#35353b] overflow-hidden">
					<div
						class="h-full rounded-full transition-all duration-300"
						style="width: {index < strengthScore
							? '100%'
							: '0%'}; background-color: {getStrengthColor(strengthScore)};"
					></div>
				</div>
			{/each}
		</div>
		<p class="text-xs mt-1 font-normal text-textActive text-left tracking-wide">
			An Ideal Passphrase should include at least
			{#each strengthResults as condition, index}
				<span class={condition.met ? "text-green-500" : "text-[#FAFC6E]"}>
					{condition.label}{index < strengthResults.length - 1 ? "," : ""}
				</span>
				{#if index < strengthResults.length - 1}&nbsp;{/if}
			{/each}
			but not mandatory.
		</p>
	</div>
</div>
