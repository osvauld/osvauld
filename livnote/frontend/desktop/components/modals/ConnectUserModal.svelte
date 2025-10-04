<script lang="ts">
	import { onMount } from "svelte";
	import { fly } from "svelte/transition";
	import { uiState, dataState } from "../../state";
	import { sendMessage } from "../../utils/helper";
	import { ClosePanel, InfoIcon } from "@osvauld/icons";

	let dialogElement: HTMLDialogElement;
	let userDetails = $state("");
	let isSubmitting = $state(false);

	const closeModal = () => {
		dialogElement?.close();
		uiState.hideConnectUserModal();
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
		userDetails = "";
	};

	const handleAddUser = async (userKey: string) => {
		try {
			await sendMessage("addKnownUser", userKey);
			uiState.showToast("User Connected successfully", true);
			closeModal();
		} catch (error) {
			uiState.showToast("Failed to connect user", false);
			console.error("Error Connecting user:", error);
		}
	};

	const handleSubmit = async (e: Event) => {
		e.preventDefault();
		if (!userDetails.trim() || isSubmitting) return;

		// Check if user is trying to add their own UserID
		try {
			const inputUserDetails = JSON.parse(atob(userDetails.trim()));
			const currentUserDetails = {
				user_public_key: dataState.userDetails?.publicKey,
				device_public_key: dataState.userDetails?.deviceKey,
				username: dataState.userDetails?.username,
			};

			if (
				JSON.stringify(inputUserDetails) === JSON.stringify(currentUserDetails)
			) {
				uiState.showToast("Cannot add your own UserID", false);
				handleClear();
				return;
			}
		} catch (error) {
			console.log("UserID format validation will be handled by backend");
		}

		isSubmitting = true;
		try {
			await handleAddUser(userDetails.trim());
			userDetails = "";
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
		class=" bg-osvauld-frameblack border border-osvauld-activeBorder rounded-3xl w-[40rem] max-w-[90vw]"
		role="dialog"
		aria-labelledby="add-user-title"
		id="connect-user-modal"
		aria-describedby="add-user-description"
		in:fly
		out:fly
	>
		<div class="h-full p-1">
			<div
				class="flex flex-col h-full max-h-[90vh] p-3 px-5 overflow-y-auto overflow-x-hidden"
			>
				<!-- Header -->
				<header class="flex justify-between items-start mb-3">
					<h2 id="add-user-title" class="text-2xl font-extralight text-white">
						Connect a User
					</h2>
					<button
						type="button"
						class="p-1 text-textActive hover:text-osvauld-sideListTextActive transition-colors rounded cursor-pointer"
						aria-label="Close Connect user modal"
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
						id="add-user-description"
						class="text-sm text-osvauld-fieldText font-light text-start"
					>
						Livnote creates a direct, peer-to-peer connection between users to
						enable secure collaboration. This private handshake requires their
						unique user address - ask them to share it with you via any medium.
						The connection activates only when both users are online. Once
						connected, you can invite them to your notes!
					</p>
				</div>

				<!-- Form Section -->
				<form onsubmit={handleSubmit} class="space-y-4">
					<div class="space-y-2">
						<label
							for="userDetails"
							class="block text-sm font-light text-white"
						>
							User Address
						</label>
						<textarea
							id="userDetails"
							bind:value={userDetails}
							placeholder="Paste user’s address here.."
							rows="6"
							required
							class="w-full px-4 py-3 text-sm bg-osvauld-frameblack border border-livnotePink rounded-lg text-white placeholder-osvauld-fieldText focus:outline-none focus:ring-1 focus:ring-livnotePink focus:border-transparent resize-none transition-colors"
							autocomplete="off"
							autocapitalize="off"
							spellcheck="false"
							onkeydown={handleKeyDown}
							aria-describedby="user-id-help"
						></textarea>
						<p id="user-id-help" class="text-xs text-osvauld-fieldText">
							User address is a publicly shareable identifier address - safe and
							meant to be shared.
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
							disabled={!userDetails.trim() || isSubmitting}
							class="px-6 py-2.5 bg-osvauld-frameblack font-normal text-sm border border-livnotePink text-livnotePink rounded-lg cursor-pointer hover:bg-livnotePink hover:text-osvauld-frameblack transition-colors"
						>
							{isSubmitting ? "Connecting..." : "Connect"}
						</button>
					</div>
				</form>
			</div>
		</div>
	</div>
</dialog>
