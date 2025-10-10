<script lang="ts">
	import { onMount } from "svelte";
	import { fly } from "svelte/transition";
	import { uiState } from "../state/ui.svelte";
	import { dataState } from "../state/data.svelte";
	import { sendMessage } from "../utils/helper";

	let dialogElement: HTMLDialogElement;
	let connectionString = $state("");
	let isSubmitting = $state(false);

	const closeModal = () => {
		dialogElement?.close();
		uiState.hideSovereignNodeModal();
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

	const handleAddSovereignNode = async (connectionStr: string) => {
		try {
			await sendMessage("addKnownUser", connectionStr);

			// Store the sovereign node user ID in dataState
			// Parse the connection string to get the user ID
			try {
				const parsedDetails = JSON.parse(atob(connectionStr.trim()));
				const sovereignNodeUserId = parsedDetails.user_public_key;
				dataState.setSovereignNodeId(sovereignNodeUserId);
				console.log("✅ Sovereign node added:", sovereignNodeUserId);
			} catch (parseError) {
				console.error("Failed to parse sovereign node details:", parseError);
			}

			// Show success message (we'll add toast later)
			console.log("✅ Sovereign node connected successfully");
			closeModal();
		} catch (error) {
			console.error("❌ Failed to connect sovereign node:", error);
		}
	};

	const handleSubmit = async (e: Event) => {
		e.preventDefault();
		if (!connectionString.trim() || isSubmitting) return;

		// Check if user is trying to add their own UserID
		try {
			const inputUserDetails = JSON.parse(atob(connectionString.trim()));
			const currentUserDetails = {
				user_public_key: dataState.userDetails?.publicKey,
				device_public_key: dataState.userDetails?.deviceKey,
				username: dataState.userDetails?.username,
			};

			if (
				JSON.stringify(inputUserDetails) === JSON.stringify(currentUserDetails)
			) {
				console.error("Cannot add your own UserID as sovereign node");
				handleClear();
				return;
			}
		} catch (error) {
			console.log("Connection string format validation will be handled by backend");
		}

		isSubmitting = true;
		try {
			await handleAddSovereignNode(connectionString.trim());
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
		aria-labelledby="sovereign-node-title"
		id="sovereign-node-modal"
		aria-describedby="sovereign-node-description"
		in:fly
		out:fly
	>
		<div class="h-full p-1">
			<div
				class="flex flex-col h-full max-h-[90vh] p-3 px-5 overflow-y-auto overflow-x-hidden"
			>
				<!-- Header -->
				<header class="flex justify-between items-start mb-3">
					<h2 id="sovereign-node-title" class="text-2xl font-normal text-white">
						Add Sovereign Node
					</h2>
					<button
						type="button"
						class="p-1 text-textActive hover:text-osvauld-sideListTextActive transition-colors rounded cursor-pointer"
						aria-label="Close sovereign node modal"
						onclick={closeModal}
					>
						<svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
							<path fill-rule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clip-rule="evenodd" />
						</svg>
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
						<svg class="w-7 h-7" fill="currentColor" viewBox="0 0 20 20">
							<path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd" />
						</svg>
					</div>
					<p
						id="sovereign-node-description"
						class="text-sm text-textActive font-normal text-start"
					>
						Connect to your sovereign node to publish your websites. The sovereign node
						acts as your personal web server, hosting your sites independently. Paste
						the connection string provided by your sovereign node to establish a secure
						connection. Once connected, you can publish and update your websites directly
						from Sthalam.
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
							placeholder="Paste sovereign node connection string here..."
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
							The connection string is provided by your sovereign node and is safe to use.
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
							{isSubmitting ? "Connecting..." : "Connect"}
						</button>
					</div>
				</form>
			</div>
		</div>
	</div>
</dialog>
