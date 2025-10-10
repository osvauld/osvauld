<script lang="ts">
	import { onMount } from "svelte";
	import { fly } from "svelte/transition";
	import { ClosePanel, InfoIcon } from "@osvauld/icons";

	interface Props {
		onClose: () => void;
	}

	let { onClose }: Props = $props();

	let dialogElement: HTMLDialogElement;
	let connectionString = $state("");
	let isSubmitting = $state(false);

	const closeModal = () => {
		dialogElement?.close();
		onClose();
	};

	const handleDialogCancel = (event: Event) => {
		event.preventDefault();
		closeModal();
	};

	const handleBackdropClick = (event: MouseEvent) => {
		if (event.target === dialogElement) {
			closeModal();
		}
	};

	const handleClear = () => {
		connectionString = "";
	};

	const handleAddWebsite = async (connString: string) => {
		try {
			console.log("Connecting to website with:", connString);
			const { sendMessage } = await import("../utils/helper");
			await sendMessage("connectToWebsite", { connectionString: connString });
			console.log("Website connection initiated successfully");
			// uiState.showToast("Website connected successfully", true);
			closeModal();
		} catch (error) {
			// uiState.showToast("Failed to connect to website", false);
			console.error("Error connecting to website:", error);
		}
	};

	const handleSubmit = async (e: Event) => {
		e.preventDefault();
		if (!connectionString.trim() || isSubmitting) return;

		isSubmitting = true;
		try {
			await handleAddWebsite(connectionString.trim());
			connectionString = "";
		} finally {
			isSubmitting = false;
		}
	};

	const handleKeyDown = (e: KeyboardEvent) => {
		if (e.key === "Enter" && !e.shiftKey) {
			e.preventDefault();
			handleSubmit(e);
		}
	};

	onMount(() => {
		dialogElement?.showModal();

		// Focus the textarea when modal opens
		const textarea = dialogElement?.querySelector(
			"textarea",
		) as HTMLTextAreaElement;
		textarea?.focus();

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
	class="m-0 max-w-none max-h-none bg-transparent"
	onclose={closeModal}
	oncancel={handleDialogCancel}
	onclick={handleBackdropClick}
>
	<div
		class="bg-osvauld-frameblack border border-osvauld-activeBorder rounded-3xl w-[40rem] max-w-[90vw]"
		role="dialog"
		aria-labelledby="add-website-title"
		id="connect-website-modal"
		aria-describedby="add-website-description"
		in:fly
		out:fly
	>
		<div class="h-full p-1">
			<div
				class="flex flex-col h-full max-h-[90vh] p-3 px-5 overflow-y-auto overflow-x-hidden"
			>
				<!-- Header -->
				<header class="flex justify-between items-start mb-3">
					<h2 id="add-website-title" class="text-2xl font-normal text-white">
						Add Website
					</h2>
					<button
						type="button"
						class="p-1 text-textActive hover:text-osvauld-sideListTextActive transition-colors rounded cursor-pointer"
						aria-label="Close Add website modal"
						onclick={closeModal}
					>
						<ClosePanel />
					</button>
				</header>

				<div
					class="h-0.5 bg-borderActive w-[calc(110%)] mx-4 ml-[-24px] mb-5 scale-y-50 origin-top-left"
				></div>

				<!-- Information Section -->
				<div
					class="flex items-start gap-3 mb-6 p-4 bg-osvauld-fieldActive rounded-lg"
				>
					<div
						class="text-livnotePink mt-0.5 flex-shrink-0 w-12 flex justify-center p-2.5"
					>
						<InfoIcon size={28} />
					</div>
					<p
						id="add-website-description"
						class="text-sm text-textActive font-normal text-start"
					>
						Connect to a published website using its connection string. Once connected,
						you'll be able to view the website and receive updates when the owner makes
						changes. The website will be saved locally and synced automatically.
					</p>
				</div>

				<!-- Form Section -->
				<form onsubmit={handleSubmit} class="space-y-4">
					<div class="space-y-2">
						<label
							for="connectionString"
							class="block text-sm font-light text-white"
						>
							Connection String
						</label>
						<textarea
							id="connectionString"
							bind:value={connectionString}
							placeholder="Paste the website connection string here..."
							rows="6"
							required
							class="w-full px-4 py-3 text-sm bg-osvauld-frameblack border border-livnotePink rounded-lg text-white placeholder-osvauld-fieldText focus:outline-none focus:ring-1 focus:ring-livnotePink focus:border-transparent resize-none transition-colors"
							autocomplete="off"
							autocapitalize="off"
							spellcheck="false"
							onkeydown={handleKeyDown}
							aria-describedby="connection-string-help"
						></textarea>
						<p id="connection-string-help" class="text-xs text-textActive">
							The connection string is provided by the website owner and is safe to share.
						</p>
					</div>

					<!-- Action Buttons -->
					<div
						class="h-0.5 bg-borderActive w-[calc(120%)] mx-4 ml-[-24px] scale-y-50 origin-top-left"
					></div>
					<div class="flex justify-end gap-3 font-light">
						<button
							type="button"
							onclick={closeModal}
							class="px-6 py-2.5 text-sm text-osvauld-fieldText hover:text-osvauld-sideListTextActive transition-colors cursor-pointer"
						>
							Cancel
						</button>
						<button
							type="submit"
							disabled={!connectionString.trim() || isSubmitting}
							class="px-6 py-2.5 bg-osvauld-frameblack font-normal text-sm border border-livnotePink text-livnotePink rounded-lg cursor-pointer hover:bg-livnotePink hover:text-osvauld-frameblack transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
						>
							{isSubmitting ? "Connecting..." : "Add Website"}
						</button>
					</div>
				</form>
			</div>
		</div>
	</div>
</dialog>
