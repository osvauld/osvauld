
<script lang="ts">
	// This Modal component can be used during exporting certificate or changing passphrase
	import { onMount } from "svelte";
	import { sendMessage, writeToClipboard } from "../utils/helper";
	import { fly } from "svelte/transition";
	import { createEventDispatcher } from "svelte";

	import { generateCertificatePDF } from "../utils/backupUtil";

	import ClosedEye from "@osvauld/password-manager-common/icons/closedEye.svelte";
	import ClosePanel from "@osvauld/password-manager-common/icons/closePanel.svelte";
	import Eye from "@osvauld/password-manager-common/icons/eye.svelte";

	import SuccessView from "./SuccessView.svelte";
	import NewPassword from "./NewPassword.svelte";
	import Loader from "./Loader.svelte";

	export let changePassword = false;
	let promptPassword = false;
	let password: string = "";
	let success: boolean = false;
	let errorView: boolean = false;
	let newPasswordView: boolean = false;
	let showPassword: boolean = false;
	let loading = false;
	const dispatch = createEventDispatcher();

	const closeModal = () => {
		dispatch("close", true);
	};

	const autofocus = (node: any) => {
		node.focus();
	};

	const newPasswordViewHandler = async () => {
		newPasswordView = true;
	};

	const handlePasswordChangeSubmit = async (e: CustomEvent) => {
		loading = true;
		const newPassword = e.detail.passphrase;
		const certificate = await sendMessage("changePassphrase", {
			oldPassword: password,
			newPassword,
		});
		if (certificate) {
			success = true;
		} else {
			errorView = true;
		}
		newPasswordView = false;
		loading = false;
		setTimeout(() => {
			closeModal();
		}, 1500);
	};

	const handleRecoveryDataSubmit = async () => {
		loading = true;
		try {
			const certificate = await sendMessage("exportCertificate", {
				passphrase: password,
			});

			if (certificate) {
				try {
					// Generate and save PDF instead of copying to clipboard
					await generateCertificatePDF(certificate);
					success = true;
				} catch (pdfError) {
					console.error("PDF generation error:", pdfError);
					errorView = true;
				}
			} else {
				errorView = true;
			}
		} catch (error) {
			console.error("Error exporting certificate:", error);
			errorView = true;
		} finally {
			loading = false;
			setTimeout(() => {
				closeModal();
			}, 1500);
		}
	};
	const handleInputChange = (e: any) => {
		password = e.target.value;
	};

	onMount(() => {
		const handleKeydown = (event: KeyboardEvent) => {
			if (event.key === "Escape") {
				closeModal();
			}
		};

		window.addEventListener("keydown", handleKeydown);

		return () => {
			window.removeEventListener("keydown", handleKeydown);
		};
	});
</script>

<div
	class="fixed inset-0 flex items-center justify-center z-50 bg-osvauld-backgroundBlur backdrop-filter backdrop-blur-[2px]"
	on:click|preventDefault="{closeModal}"
	role="presentation">
	<div
		class="p-4 bg-osvauld-frameblack border border-osvauld-activeBorder rounded-3xl w-[32rem] h-[32rem] flex flex-col justify-center items-center"
		on:click|stopPropagation
		role="presentation"
		aria-labelledby="export-recovery-data"
		in:fly
		out:fly>
		{#if loading}
			<Loader color="#fff" size="{32}" />
		{:else if errorView}
			<SuccessView status="{false}" message="Unable to do operation" />
		{:else if success}
			<SuccessView
				status="{true}"
				message="{changePassword ? 'Password Changed' : 'Export complete'}" />
		{:else if newPasswordView}
			<NewPassword on:submit="{handlePasswordChangeSubmit}" />
		{:else}
			<form
				class="flex flex-col items-center h-full w-full"
				on:submit|preventDefault="{changePassword
					? newPasswordViewHandler
					: handleRecoveryDataSubmit}">
				<div class="flex p-2 pb-4 justify-between items-center w-full">
					<span
						id="export-recovery-data"
						class="text-[21px] font-medium text-osvauld-quarzowhite">
						Confirm Passphrase
					</span>
					<button
						class="cursor-pointer p-2"
						type="button"
						on:click|preventDefault="{closeModal}"
						aria-label="Close">
						<ClosePanel />
					</button>
				</div>
				<div class="border-b border-osvauld-iconblack w-[90%]"></div>
				<div class="grow flex justify-center items-center">
					<div
						class="flex justify-between items-center bg-osvauld-frameblack px-3 border rounded-lg border-osvauld-iconblack focus-within:border-osvauld-activeBorder">
						<input
							class="text-white p-2 bg-osvauld-frameblack border-0 tracking-wider font-normal border-transparent focus:ring-0 focus:border-osvauld-activeBorder focus:outline-none"
							type="{showPassword ? 'text' : 'password'}"
							id="passphrase"
							aria-label="passphrase"
							autocomplete="off"
							use:autofocus
							on:input="{handleInputChange}" />

						<button
							type="button"
							class="flex justify-center items-center"
							on:click="{() => (showPassword = !showPassword)}">
							{#if showPassword}
								<ClosedEye />
							{:else}
								<Eye />
							{/if}
						</button>
					</div>
				</div>
				<button
					class="border w-[10rem] py-3 px-6 my-4 mx-auto text-base font-medium rounded-md bg-osvauld-carolinablue border-osvauld-carolinablue text-osvauld-frameblack cursor-pointer"
					type="submit"
					disabled="{!password}">
					Proceed
				</button>
			</form>
		{/if}
	</div>
</div>
