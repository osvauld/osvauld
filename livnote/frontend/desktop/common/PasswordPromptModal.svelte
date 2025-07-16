
	<script lang="ts">
	// This Modal component can be used during exporting certificate or changing passphrase
	import { onMount } from "svelte";
	import { sendMessage } from "../utils/helper";
	import { fly } from "svelte/transition";
	import { generateCertificatePDF } from "../utils/backupUtil";
	import { ClosedEye, Eye } from "../icons";
	import SuccessView from "./SuccessView.svelte";
	import NewPassword from "./NewPassword.svelte";
	import Loader from "./Loader.svelte";

	// Replace props and event dispatch with callback props
	let { changePassword = false, onClose } = $props();

	// State variables
	let password = $state("");
	let success = $state(false);
	let errorView = $state(false);
	let newPasswordView = $state(false);
	let showPassword = $state(false);
	let parentLoaderActive = $state(false);
	let isLoaderActive = $state(false);
	let dialogElement: HTMLDialogElement;

	const closeModal = () => {
		dialogElement?.close();
		onClose?.(true);
	};


	const delay = async(time: number) => {
		return new Promise(resolve => setTimeout(resolve, time));
	}


	const newPasswordViewHandler = async (event: Event) => {
		event.preventDefault();
		newPasswordView = true;
	};

	const handlePasswordChangeSubmit = async (passphrase: string) => {
		const newPassword = passphrase;
		try {
			await sendMessage("changePassphrase", {
				oldPassword: password,
				newPassword,
			});
		} catch (error) {
			console.error("Error changing passphrase:  ////>>>>>", error);
			errorView = true;
		} finally {
			await delay(1000);
			if(!errorView) success = true;
			isLoaderActive = false;
			newPasswordView = false;
			await delay(2000);
			closeModal();
		}
	};

	const handleExportPdfSubmit = async (event: Event) => {
		event.preventDefault();

		parentLoaderActive = true;
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
			parentLoaderActive = false;
			setTimeout(() => {
				closeModal();
			}, 1500);
		}
	};


	const handleInputChange = (e: Event) => {
		if (e.target instanceof HTMLInputElement) {
			password = e.target.value;
		}
	};

	// Handle backdrop clicks via the native dialog cancel event
	const handleDialogCancel = (event: Event) => {
		event.preventDefault(); // Prevent default cancel behavior
		closeModal();
	};

	// Handle backdrop clicks for mouse users
	const handleBackdropClick = (event: MouseEvent) => {
		// Only close if clicking directly on the dialog element (backdrop area)
		if (event.target === dialogElement) {
			closeModal();
		}
	};

	onMount(() => {
		// Show the dialog as modal (provides built-in focus trapping)
		dialogElement?.showModal();

		// Focus the first input when dialog opens
		const firstInput = dialogElement?.querySelector('input[type="password"], input[type="text"]') as HTMLInputElement;
		firstInput?.focus();

		return () => {
			dialogElement?.close();
		};
	});
</script>

<style>
	dialog {
		position: fixed;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		border: none;
		padding: 0;
	}
	
	dialog::backdrop {
		background: rgba(0, 0, 0, 0.5);
		backdrop-filter: blur(2px);
	}
</style>

<dialog
	bind:this={dialogElement}
	class="m-0 max-w-none max-h-none bg-transparent "
	onclose={closeModal}
	oncancel={handleDialogCancel}
	onclick={handleBackdropClick}>
	<div
		class="p-4 bg-bgPrimary border border-osvauld-activeBorder rounded-xl w-[42rem] h-[42rem] flex flex-col justify-center items-center"
		role="document"
		aria-labelledby="confirm-passphrase"
		in:fly
		out:fly>
		{#if parentLoaderActive}
			<Loader color="#fff" size={32} />
		{:else if errorView}
			<SuccessView status={false} message="Unable to do operation" />
		{:else if success}
			<SuccessView
				status={true}
				message={changePassword ? "Password Changed" : "Export complete"} />
		{:else if newPasswordView}
			<NewPassword
			isLoaderActive={isLoaderActive}
			onReturn={handlePasswordChangeSubmit}
			/>
		{:else}
			<form
				class="flex flex-col items-center h-full w-full"
				onsubmit={changePassword
					? newPasswordViewHandler
					: handleExportPdfSubmit}>
				<header class="flex p-2 pb-4 justify-center items-center w-full">
					<h2
						id="confirm-passphrase"
						class="text-[21px] font-normal text-white ">
						Confirm Passphrase
					</h2>
				</header>
				<main class="grow flex justify-center items-center">
					<fieldset class="border-0 p-0 m-0">
						<label
						for="passphrase"
						class="font-normal text-white self-start "
						>Enter passphrase</label>
						<div
							class="w-[20rem] flex justify-between items-center bg-bgPrimary px-3 border rounded-lg border-osvauld-iconblack focus-within:border-livnotePink mt-2">
					
							<input
								class="text-white h-[3.3rem]  p-2 bg-bgPrimary border-0 tracking-wider font-normal border-transparent focus:ring-0 focus:outline-none"
								type={showPassword ? "text" : "password"}
								id="passphrase"
								name="passphrase"
								autocomplete="current-password"
								required
								oninput={handleInputChange} />

							<button
								type="button"
								class="flex justify-center items-center"
								aria-label={showPassword ? "Hide password" : "Show password"}
								onclick={() => (showPassword = !showPassword)}>
								{#if showPassword}
									<ClosedEye />
								{:else}
									<Eye />
								{/if}
							</button>
						</div>
					</fieldset>
				</main>
				<footer class="flex justify-center">
					<button
						class="border w-[20rem] py-3 px-6 my-4 mx-auto text-base font-medium rounded-md bg-livnotePink text-primarydark cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
						type="submit"
						disabled={!password}>
						Proceed
					</button>
				</footer>
			</form>
		{/if}
	</div>
</dialog> 