<script>
	import Add from "@osvauld/password-manager-common/icons/add.svelte";
	import EmptyPlaceholder from "@osvauld/password-manager-common/icons/mobileEmptyPlaceholder.svelte";
</script>

<div
	class="h-full text-mobile-textPrimary px-2.5 py-4 box-border flex flex-col gap-3">
	<div class="h-[112px] rounded-xl bg-mobile-bgSeconary p-4">
		<h2 class="text-mobile-textActive font-normal text-xl leading-6">
			Welcome <br />
			Tony Antony
		</h2>
		<p class="text-mobile-textSecondary text-base mt-4">
			Let’s jump right into things!
		</p>
	</div>

	<div class=" rounded-xl bg-mobile-bgSeconary p-4 grow flex flex-col">
		<h2 class="text-mobile-textActive font-normal text-xl leading-6">
			Organize your <br /> secret credentials
		</h2>
		<p class="text-mobile-textSecondary text-base mt-4">
			Create a new vault to securely store all your secrets. Stay organized and
			access them easily whenever you need.
		</p>

		<div class="w-full h-[300px] bg-cover">
			<EmptyPlaceholder />
		</div>
		<button
			class="mt-auto text-lg px-10 py-2.5 bg-osvauld-carolinablue text-mobile-bgPrimary flex items-center justify-center rounded-lg font-medium">
			Create New Secret
			<Add />
		</button>
	</div>
</div>
