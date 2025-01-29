<script lang="ts">
	import {
		currentCredential,
		viewCredentialModal,
		currentVault,
		deleteConfirmationModal,
		vaults,
		refreshCredentialList,
	} from "../store/desktop.ui.store";

	import { scale } from "svelte/transition";

	import Tick from "../../../icons/tick.svelte";
	import CopyIcon from "../../../icons/copyIcon.svelte";
	import More from "../../../icons/more.svelte";
	import Edit from "../../../icons/editIcon.svelte";
	import MobileHome from "../../../icons/mobileHome.svelte";
	import ClosePanel from "../../../icons/closePanel.svelte";
	import Share from "../../../icons/FolderShare.svelte";
	import Star from "../../../icons/star.svelte";
	import Bin from "../../../icons/binIcon.svelte";
	import ClosedEye from "../../../icons/closedEye.svelte";
	import Eye from "../../../icons/eye.svelte";
	import FavStar from "../../../icons/favStar.svelte";

	import { CATEGORIES, sensitiveFieldNames } from "../../utils/credentialUtils";

	import {
		writeToClipboard,
		sendMessage,
	} from "../../../lib/components/dashboard/helper";
	import { onMount } from "svelte";

	let copied = false;
	let copiedItemIndex;
	let deleteHovered = false;
	let showPassword = false;
	let favourite = $currentCredential?.favourite || false;

	let type = $currentCredential.data.credentialType;
	let categoryInfo = CATEGORIES.find((item) => item.type === type);
	let folderName = $vaults.find(
		(vault) => vault.id === $currentCredential?.folder_id,
	)?.name;

	const togglePassword = () => {
		showPassword = !showPassword;
	};

	const CloseViewModal = () => {
		viewCredentialModal.set(false);
		currentCredential.set({});
	};

	const copyToClipboard = async (fieldValue, index) => {
		copiedItemIndex = index;
		copied = true;

		writeToClipboard(fieldValue);
		setTimeout(() => {
			copied = false;
			copiedItemIndex = null;
		}, 1000);
	};

	const handleCredentialDelete = () => {
		viewCredentialModal.set(false);
		deleteConfirmationModal.set({ item: "credential", show: true });
	};

	const toggleFavorite = async (id) => {
		favourite = !favourite;
		await sendMessage("toggleFav", { credentialId: id });
		refreshCredentialList.set(true);
	};
</script>

<div
	class="fixed inset-0 z-50 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[0.5px] flex justify-center items-center">
	<div
		class="w-[40rem] h-[50rem] flex flex-col bg-osvauld-ninjablack border border-osvauld-activeBorder rounded-3xl p-10">
		<div
			class="w-full h-[48px] px-3 flex justify-start items-center gap-2 flex-shrink-0">
			<span class=" p-2.5 rounded-lg bg-mobile-bgSeconary"
				>{#if categoryInfo && categoryInfo.icon}
					<svelte:component this="{categoryInfo.icon}" color="{'#BFC0CC'}" />
				{:else}
					<span>!</span>
				{/if}</span>
			<button
				class="ml-auto p-2.5 rounded-lg bg-mobile-bgSeconary flex justify-center items-center active:scale-95"
				on:click|stopPropagation="{() =>
					toggleFavorite($currentCredential.id)}">
				{#if favourite}
					<FavStar />
				{:else}
					<Star />
				{/if}
			</button>
			<button
				class="p-2.5 rounded-lg bg-mobile-bgSeconary"
				on:mouseenter="{() => (deleteHovered = true)}"
				on:mouseleave="{() => (deleteHovered = false)}"
				on:click|stopPropagation="{handleCredentialDelete}"
				><Bin
					color="{deleteHovered ? '#FF6A6A' : '#85889C'}"
					size="{24}" /></button>
			<span class=" p-2.5 rounded-lg bg-mobile-bgSeconary"
				><Edit size="{24}" /></span>
			<button
				class="p-2.5 rounded-lg bg-mobile-bgSeconary"
				on:click="{CloseViewModal}"><ClosePanel size="{24}" /></button>
		</div>
		<div
			class="text-mobile-textPrimary text-2xl font-medium flex flex-col pl-4 py-3">
			<span class="text-mobile-textTertiary"
				>{$currentCredential.data.credentialType}</span>
			<div
				class="flex text-sm font-light gap-1 items-center tracking-wider capitalize">
				<span><MobileHome size="{14}" color="{'#85889C'}" /></span>
				<span>{folderName}</span>
			</div>
		</div>
		<hr class="border border-mobile-bgSeconary my-3" />
		<div
			class="grow p-4 text-mobile-textPrimary overflow-y-auto scrollbar-thin flex flex-col gap-2 mt-2">
			{#if type === "Note"}
				{@const value = $currentCredential.data.credentialFields.find(
					(item) => item.fieldName === "Note",
				)?.fieldValue}
				<div class="w-full h-full flex flex-col gap-4">
					<div
						class="bg-osvauld-fieldActive rounded-lg py-2.5 px-4 w-full h-full max-h-full overflow-y-scroll">
						<p
							class="text-left text-mobile-textField text-base text-wrap break-all">
							{value || "Regret"}
						</p>
					</div>
					<button
						class="bg-osvauld-fieldActive rounded-lg p-3 w-full flex gap-2 justify-center items-center"
						on:click|preventDefault|stopPropagation="{() =>
							copyToClipboard(value, 0)}">
						<span
							class="text-osvauld-fieldText font-Jakarta font-medium text-lg"
							>Copy note</span>
						{#if copied && copiedItemIndex === 0}
							<span in:scale>
								<Tick />
							</span>
						{:else}
							<CopyIcon color="{'#85889C'}" />
						{/if}
					</button>
				</div>
			{:else}
				{#each $currentCredential.data.credentialFields as field, index (field.fieldName)}
					{#if field.fieldValue.trim().length !== 0}
						<span class="text-mobile-textlabel text-sm">{field.fieldName}</span>
						<div class="flex gap-2">
							{#if sensitiveFieldNames.includes(field.fieldName)}
								<!-- If field name is sensitive, Hide -->
								<div
									class="bg-osvauld-fieldActive rounded-lg py-1 px-4 w-full flex"
									on:click|stopPropagation>
									<input
										type="{showPassword ? 'text' : 'password'}"
										class="w-5/6 text-left p-0 leading-3 bg-osvauld-fieldActive text-mobile-textField text-base border-0 outline-0 focus:ring-0 truncate"
										value="{showPassword ? field.fieldValue : '••••••••'}" />
									<button
										type="button"
										class="ml-auto flex-none flex justify-center items-center"
										on:click="{() => togglePassword()}">
										{#if showPassword}
											<ClosedEye />
										{:else}
											<Eye />
										{/if}
									</button>
								</div>
								<button
									class="bg-osvauld-fieldActive rounded-lg p-3"
									on:click|preventDefault|stopPropagation="{() =>
										copyToClipboard(field.fieldValue, index)}">
									{#if copied && copiedItemIndex === index}
										<span in:scale>
											<Tick />
										</span>
									{:else}
										<CopyIcon color="{'#85889C'}" />
									{/if}
								</button>
							{:else}
								<div
									class="bg-mobile-bgSeconary rounded-lg py-2.5 px-4 flex-1 max-w-full truncate">
									<span class=" text-mobile-textField text-base"
										>{field.fieldValue}</span>
								</div>
								<button
									class="bg-mobile-bgSeconary rounded-lg p-3"
									on:click|preventDefault|stopPropagation="{() =>
										copyToClipboard(field.fieldValue, index)}">
									{#if copied && copiedItemIndex === index}
										<span in:scale>
											<Tick />
										</span>
									{:else}
										<CopyIcon color="{'#85889C'}" />
									{/if}
								</button>
							{/if}
						</div>
					{/if}
				{/each}
			{/if}

			<div
				class="bg-mobile-bgSeconary rounded-lg h-[65px] p-3 text-mobile-textPrimary text-sm mt-9 flex justify-start items-center">
				<span>
					Last accessed at : <br />
					<span class="text-xs"
						>{new Date($currentCredential.last_accessed).toLocaleTimeString() +
							"," +
							new Date(
								$currentCredential.last_accessed,
							).toLocaleDateString()}</span>
				</span>
			</div>
		</div>
	</div>
</div>
