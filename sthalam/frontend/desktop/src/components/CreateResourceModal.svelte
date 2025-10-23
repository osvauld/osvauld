<script lang="ts">
	import { onMount } from "svelte";
	import { fly } from "svelte/transition";
	import { dataState } from "../state/data.svelte";

	interface Props {
		websiteId: string;
		onClose: () => void;
	}

	let { websiteId, onClose }: Props = $props();

	let dialogElement: HTMLDialogElement;
	let title = $state("");
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

	const handleSubmit = async (e: Event) => {
		e.preventDefault();
		if (!title.trim() || isSubmitting) return;

		isSubmitting = true;
		try {
			await dataState.addResource(websiteId, title.trim(), "website");
			closeModal();
		} catch (error) {
			console.error("Failed to create resource:", error);
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

		// Focus the title input when modal opens
		const input = dialogElement?.querySelector("input") as HTMLInputElement;
		input?.focus();

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
		class="bg-osvauld-frameblack border border-osvauld-activeBorder rounded-3xl w-[36rem] max-w-[90vw]"
		role="dialog"
		aria-labelledby="create-resource-title"
		id="create-resource-modal"
		in:fly
		out:fly
	>
		<div class="h-full p-1">
			<div
				class="flex flex-col h-full max-h-[90vh] p-3 px-5 overflow-y-auto overflow-x-hidden"
			>
				<!-- Header -->
				<header class="flex justify-between items-start mb-3">
					<h2 id="create-resource-title" class="text-2xl font-normal text-white">
						Create New Page
					</h2>
					<button
						type="button"
						class="p-1 text-textActive hover:text-osvauld-sideListTextActive transition-colors rounded cursor-pointer"
						aria-label="Close create resource modal"
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

				<!-- Form Section -->
				<form onsubmit={handleSubmit} class="space-y-5">
					<!-- Title Input -->
					<div class="space-y-2">
						<label
							for="title"
							class="block text-sm font-light text-white"
						>
							Page Title
						</label>
						<input
							id="title"
							type="text"
							bind:value={title}
							placeholder="Enter page title..."
							required
							class="w-full px-4 py-3 text-sm bg-osvauld-frameblack border border-livnotePink rounded-lg text-white placeholder-osvauld-fieldText focus:outline-none focus:ring-1 focus:ring-livnotePink focus:border-transparent transition-colors"
							autocomplete="off"
							onkeydown={handleKeyDown}
						/>
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
							disabled={!title.trim() || isSubmitting}
							class="px-6 py-2.5 bg-osvauld-frameblack font-normal text-sm border border-livnotePink text-livnotePink rounded-lg cursor-pointer hover:bg-livnotePink hover:text-osvauld-frameblack transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
						>
							{isSubmitting ? "Creating..." : "Create"}
						</button>
					</div>
				</form>
			</div>
		</div>
	</div>
</dialog>
