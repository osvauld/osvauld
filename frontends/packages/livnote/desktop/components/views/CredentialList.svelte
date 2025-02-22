<script lang="ts">
	import {
		selectedCategory,
		currentVault,
		credentialEditorModal,
		viewCredentialModal,
		currentCredential,
		deleteConfirmationModal,
		refreshCredentialList,
		noteViewLayout,
	} from "../../store/desktop.ui.store";
	import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
	import Import from "@osvauld/password-manager-common/icons/import.svelte";
	import ImportModal from "./ImportModal.svelte";
	import CredentialCard from "../ui/CredentialCard.svelte";
	import LL from "@osvauld/password-manager-common//i18n/i18n-svelte";
	import DocumentEditor from "../lib/DocumentEditor.svelte";

	let clickTimer = null;
	let clickDelay = 200;
	let credentials = [];
	let prevDeleteModalState = false;
	let prevImportModalState = false;
	let importHovered = false;
	let importSelected = false;
	let credentialcardstates = []; // { id: null, show: false }[];

	let updatedCredentials = [
		{ id: 1, favourite: true },
		{ id: 2, favourite: false },
		{ id: 3, favourite: true },
		{ id: 4, favourite: false },
		{ id: 5, favourite: true },
		{ id: 6, favourite: false },
		{ id: 7, favourite: true },
		{ id: 8, favourite: false },
		{ id: 9, favourite: true },
		{ id: 10, favourite: false },
		{ id: 11, favourite: true },
	];

	// const fetchCredentials = async (vaultId: string) => {
	// 	try {
	// 		credentials = await sendMessage("getCredentialsForFolder", {
	// 			folderId: vaultId,
	// 		});
	// 	} catch (error) {
	// 		credentials = [];
	// 	}
	// };

	// const fetchAllCredentials = async () => {
	// 	try {
	// 		credentials = await sendMessage("getAllCredentials", {
	// 			favourite: false,
	// 		});
	// 	} catch (error) {
	// 		credentials = [];
	// 	}
	// };

	// // I need this to fetch again when the import modal is closed
	// $: {
	// 	fetchCredentials($currentVault.id);
	// 	credentialcardstates = [];
	// }

	// $: if ($refreshCredentialList) {
	// 	if ($currentVault.id === "all") {
	// 		fetchAllCredentials();
	// 	} else {
	// 		fetchCredentials($currentVault.id);
	// 	}
	// 	refreshCredentialList.set(false);
	// }

	// $: if ($currentVault.id === "all") {
	// 	fetchAllCredentials();
	// 	credentialcardstates = [];
	// }

	// $: {
	// 	if (prevDeleteModalState && !$deleteConfirmationModal.show) {
	// 		if ($currentVault.id === "all") {
	// 			fetchAllCredentials();
	// 		} else {
	// 			fetchCredentials($currentVault.id);
	// 		}
	// 	}
	// 	prevDeleteModalState = $deleteConfirmationModal.show;
	// }
	// $: {
	// 	if (prevImportModalState && !importSelected) {
	// 		fetchCredentials($currentVault.id);
	// 	}
	// 	prevImportModalState = importSelected;
	// }

	// $: updatedCredentials = $selectedCategory
	// 	? $selectedCategory === "favourites"
	// 		? credentials.filter((credential) => credential.favourite)
	// 		: credentials.filter(
	// 				(credential) => credential.data.credentialType === $selectedCategory,
	// 			)
	// 	: credentials;

	const selectedCredential = (credential) => {
		// viewCredentialModal.set(true);
		// currentCredential.set(credential);
	};

	// const handleExpand = (id) => {
	// 	const index = credentialcardstates.findIndex((item) => item.id === id);
	// 	if (index !== -1) {
	// 		credentialcardstates = credentialcardstates.map((item) =>
	// 			item.id === id ? { ...item, show: !item.show } : item,
	// 		);
	// 	} else {
	// 		credentialcardstates = [...credentialcardstates, { id, show: true }];
	// 	}
	// };

	// const closeImportModal = async () => {
	// 	importSelected = false;
	// };

	const getColumnCount = () => {
		if (typeof window === "undefined") return 1;
		if (window.innerWidth >= 1024) return 3;
		if (window.innerWidth >= 640) return 2;
		return 1;
	};

	const getColumnItems = (items, colIndex) => {
		const colCount = getColumnCount();
		return items.filter((_, index) => index % colCount === colIndex);
	};

	const handleClick = (e) => {
		// selectedCredential(e.detail);
		noteViewLayout.set(true);
	};
</script>

<!-- <div class="grow max-h-[85%] px-16 py-4 relative">
	{#if importSelected}
		<div
			class="fixed inset-0 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[2px] flex items-center justify-center z-50">
			<ImportModal on:close="{closeImportModal}" />
		</div>
	{/if}
	<div class="h-full overflow-y-auto overflow-x-hidden pr-1 scrollbar-none">
		{#key updatedCredentials}
			<div class="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-3">
				{#each Array(getColumnCount()) as _, colIndex}
					<div class="flex flex-col gap-3">
						{#each getColumnItems(updatedCredentials, colIndex) as credential (credential.id)}
							<CredentialCard
								{credential}
								{credentialcardstates}
								on:dbl="{handleDoubleClick}"
								on:clk="{handleClick}" />
						{/each}
					</div>
				{/each}
			</div>
		{/key}
	</div>
	{#if $currentVault.id !== "all"}
		<button
			class="text-lg absolute bottom-10 right-14 bg-osvauld-frameblack border border-osvauld-iconblack text-osvauld-sheffieldgrey hover:bg-osvauld-carolinablue hover:text-osvauld-ninjablack rounded-lg py-2 px-3.5 flex justify-center items-center"
			type="button"
			on:mouseenter="{() => (importHovered = true)}"
			on:mouseleave="{() => (importHovered = false)}"
			on:click="{() => (importSelected = true)}">
			<Import color="{importHovered ? '#0D0E13' : '#6E7681'}" />
			<span class="ml-2">{$LL.import()}</span>
		</button>
	{/if}
</div> -->
<div class="grow max-h-[85%] px-16 py-4 relative">
	<div class="h-full overflow-y-auto overflow-x-hidden pr-1 scrollbar-none">
		{#if $noteViewLayout}
			<DocumentEditor />
		{:else}
			{#key updatedCredentials}
				<div class="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-3">
					{#each Array(getColumnCount()) as _, colIndex}
						<div class="flex flex-col gap-3">
							{#each getColumnItems(updatedCredentials, colIndex) as credential (credential.id)}
								<CredentialCard
									{credential}
									{credentialcardstates}
									on:clk="{handleClick}" />
							{/each}
						</div>
					{/each}
				</div>
			{/key}
		{/if}
	</div>
</div>
