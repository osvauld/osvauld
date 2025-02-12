<script lang="ts">
	import { createEventDispatcher } from "svelte";
	import type { IntermediateCredential } from "../../dtos/import.dto";
	import { currentVault } from "../store/desktop.ui.store";

	export let dataFromParser: IntermediateCredential[];

	const dispatch = createEventDispatcher();

	let selectedCredentials: Record<string, boolean> = {};
	let selectionLength: number | undefined;

	$: {
		dataFromParser.forEach((_, index) => {
			if (!(index in selectedCredentials)) {
				selectedCredentials[index] = true;
			}
		});
		selectionLength = Object.values(selectedCredentials).filter(
			(checked) => checked === true,
		).length;
	}

	const handleProceed = () => {
		const filteredData = dataFromParser.filter(
			(_, index) => selectedCredentials[index],
		);
		if ($currentVault.id) {
			dispatch("approved", { ...filteredData, folderId: $currentVault.id });
		}
	};

	const changeSelection = (index: number) => {
		selectedCredentials[index] = !selectedCredentials[index];
	};
</script>

<div class="w-full h-full overflow-y-auto overflow-x-hidden scrollbar-thin">
	<table class="w-full text-sm text-left text-osvauld-chalkwhite">
		<thead class="text-xs uppercase sticky top-0 bg-osvauld-frameblack">
			<tr>
				<th class="px-6 py-3 flex flex-col justify-center items-center"
					>Select <span>({selectionLength})</span></th>
				<th class="px-6 py-3">Name</th>
				<th class="px-6 py-3">Username</th>
				<th class="px-6 py-3">Domain</th>
			</tr>
		</thead>
		<tbody>
			{#each dataFromParser as cred, index}
				<tr class="border-b border-osvauld-iconblack">
					<td
						class="px-6 py-4 text-center cursor-pointer"
						on:click="{() => changeSelection(index)}">
						<input
							type="checkbox"
							bind:checked="{selectedCredentials[index]}"
							class="w-5 h-5 text-osvauld-carolinablue bg-osvauld-iconblack border border-osvauld-sheffieldgrey rounded active:outline-none focus:ring-offset-0 focus:ring-0 cursor-pointer" />
					</td>
					<td class="px-6 py-4">{cred.name}</td>
					<td class="px-6 py-4">{cred.username}</td>
					<td class="px-6 py-4">{cred.domain}</td>
				</tr>
			{/each}
		</tbody>
	</table>
</div>

<div class="flex justify-center mt-4">
	<button
		on:click="{handleProceed}"
		class="px-6 py-2 bg-osvauld-carolinablue text-osvauld-frameblack rounded-lg
           hover:bg-osvauld-frameblack
					hover:text-osvauld-carolinablue
           border border-osvauld-carolinablue transition-all duration-300 ease-in-out">
		Proceed
	</button>
</div>
