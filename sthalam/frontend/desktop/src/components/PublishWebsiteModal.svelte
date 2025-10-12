<script lang="ts">
	import { dataState } from "../state/data.svelte";
	import { sendMessage } from "../utils/helper";
	import { onMount, onDestroy } from "svelte";
	import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";

	interface User {
		username: string;
		id: string;
	}

	interface Props {
		show?: boolean;
		onClose?: () => void;
	}

	let { show = $bindable(false), onClose = () => {} }: Props = $props();

	let availableUsers = $state<User[]>([]);
	let existingUsers = $state<User[]>([]);
	let selectedUserId = $state<string | null>(null);
	let isPublishing = $state(false);
	let isGeneratingLink = $state(false);
	let generatedConnectionString = $state<string | null>(null);
	let selectedSovereignNodeId = $state<string | null>(null);
	let isCopied = $state(false);
	let unlisten: UnlistenFn | null = null;

	async function fetchUsers() {
		try {
			const users = await sendMessage("getKnownUsers");
			availableUsers = users || [];

			console.log("📋 Available users:", availableUsers);
			console.log("🌐 Sovereign node ID:", dataState.sovereignNodeId);

			// Fetch users this folder is already shared with
			if (dataState.currentWebsite && dataState.currentWebsite.id !== "all") {
				const sharedUsers = await sendMessage("getSharedFolderUsers", {
					folderId: dataState.currentWebsite.id
				});
				existingUsers = sharedUsers || [];
				console.log("✅ Existing users for folder:", existingUsers);
			}
		} catch (error) {
			console.error("Error fetching users:", error);
		}
	}

	const generateFolderPermissions = (folderId: string) => {
		const abilities = [
			"crud/read",
			"crud/update",
			"crud/delete",
			"add_resources",
			"share_folder",
		];
		const folderURI = `sthalam:folder:${folderId}`;
		const permissionsToGrant = abilities.map((ability) => [folderURI, ability]);
		return permissionsToGrant;
	};

	const handleGenerateLink = async () => {
		const currentWebsite = dataState.currentWebsite;
		if (!currentWebsite || currentWebsite.id === "all") {
			console.error("No website selected");
			return;
		}

		if (!selectedSovereignNodeId) {
			console.error("No sovereign node selected");
			// TODO: Show error toast
			return;
		}

		// First, get the device_id for the selected user
		const selectedUser = existingUsers.find(u => u.id === selectedSovereignNodeId);
		if (!selectedUser) {
			console.error("Selected sovereign node not found");
			return;
		}

		isGeneratingLink = true;
		generatedConnectionString = null; // Reset previous string
		isCopied = false;
		try {
			// Emit event to backend with the device_id (which is the same as user.id in this context)
			await emit("request-folder-token", {
				folderId: currentWebsite.id,
				deviceId: selectedUser.id, // This is actually the device_id from the User object
				domain: "sthalam"
			});

			console.log("✅ Folder token request event emitted");
		} catch (error) {
			console.error("❌ Failed to emit folder token request:", error);
			// TODO: Show error toast
			isGeneratingLink = false;
		}
	};

	const handleCopyConnectionString = async () => {
		if (!generatedConnectionString) return;

		try {
			await navigator.clipboard.writeText(generatedConnectionString);
			isCopied = true;
			setTimeout(() => {
				isCopied = false;
			}, 2000);
		} catch (error) {
			console.error("Failed to copy connection string:", error);
		}
	};

	const handlePublish = async () => {
		if (!selectedUserId) {
			console.error("No user selected");
			return;
		}

		const currentWebsite = dataState.currentWebsite;
		if (!currentWebsite || currentWebsite.id === "all") {
			console.error("No website selected");
			return;
		}

		isPublishing = true;
		try {
			const permissions = generateFolderPermissions(currentWebsite.id);
			await sendMessage("shareFolder", {
				folderId: currentWebsite.id,
				userId: selectedUserId,
				permissions,
			});

			console.log("✅ Website published successfully!");
			// TODO: Show success toast

			// Refresh shared folder users
			await fetchUsers();

			// Close modal
			selectedUserId = null;
			onClose();
		} catch (error) {
			console.error("❌ Failed to publish website:", error);
			// TODO: Show error toast
		} finally {
			isPublishing = false;
		}
	};

	$effect(() => {
		if (show) {
			fetchUsers();
			selectedUserId = null;
		}
	});

	onMount(async () => {
		// Listen for folder token response
		unlisten = await listen("folder-token-received", (event: any) => {
			console.log("📥 Received folder token:", event.payload);
			const { folderId, connectionString } = event.payload;

			// Only process if it's for the current website
			if (folderId === dataState.currentWebsite?.id) {
				generatedConnectionString = connectionString;
				isGeneratingLink = false;
				console.log("✅ Connection string received and displayed");
			}
		});
	});

	onDestroy(() => {
		// Cleanup event listener
		if (unlisten) {
			unlisten();
		}
	});
</script>

{#if show}
	<!-- Backdrop -->
	<div
		class="fixed inset-0 bg-black/50 backdrop-blur-sm z-[999]"
		onclick={onClose}
	></div>

	<!-- Modal -->
	<div
		class="fixed top-1/2 left-1/2 transform -translate-x-1/2 -translate-y-1/2 w-[30rem] max-w-[90vw] max-h-[80vh] rounded-2xl border border-osvauld-activeBorder bg-osvauld-frameblack flex flex-col z-[1000]"
		role="dialog"
		aria-labelledby="publish-title"
	>
		<!-- Header (fixed at top) -->
		<div class="flex justify-between items-center p-6 pb-4">
			<h2 id="publish-title" class="text-xl text-white font-normal">
				Publish Website
			</h2>
			<button
				class="p-2 rounded-lg hover:bg-osvauld-fieldActive transition-colors"
				aria-label="Close"
				onclick={onClose}
			>
				<svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
					<path fill-rule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clip-rule="evenodd" />
				</svg>
			</button>
		</div>

		<!-- Scrollable content area -->
		<div class="flex-1 overflow-y-auto px-6">
			<p class="text-sm text-textActive mb-4">
				Select a user to publish "{dataState.currentWebsite?.name}" to:
			</p>

		<!-- Existing Users (Already Published To) -->
		{#if existingUsers.length > 0}
			<div class="mb-4">
				<h3 class="text-sm text-textActive mb-2">Already published to:</h3>
				<div class="space-y-2 max-h-[10rem] overflow-y-auto">
					{#each existingUsers as user}
						{@const isSovereignNode = user.id === dataState.sovereignNodeId}
						<div class="flex items-center gap-3 px-3 py-2 bg-osvauld-fieldActive rounded-lg">
							<div class="w-8 h-8 rounded-full bg-livnotePink flex items-center justify-center text-black font-medium">
								{user.username.charAt(0).toUpperCase()}
							</div>
							<span class="text-white flex items-center gap-2">
								{user.username}
								{#if isSovereignNode}
									<span class="text-xs px-2 py-0.5 bg-livnotePink/20 text-livnotePink rounded-full border border-livnotePink/40">Node</span>
								{/if}
							</span>
							<span class="ml-auto text-xs text-green-500">Published</span>
						</div>
					{/each}
				</div>
			</div>
			<div class="h-px bg-osvauld-borderColor my-4"></div>
		{/if}

		<!-- Available Users -->
		<div class="flex-1 overflow-y-auto min-h-[8rem]">
			<h3 class="text-sm text-textActive mb-2">Available users: ({availableUsers.length})</h3>
			{#if availableUsers.length === 0}
				<p class="text-sm text-textActive py-4">No users available. Add a user first.</p>
			{:else}
				<div class="space-y-2">
					{#each availableUsers as user}
						{@const isAlreadyPublished = existingUsers.some(u => u.id === user.id)}
						{@const isSovereignNode = user.id === dataState.sovereignNodeId}
						{(() => {
							console.log("🔍 Rendering user:", user.username, "ID:", user.id, "Already published:", isAlreadyPublished);
							return "";
						})()}
						<button
							class="w-full flex items-center gap-3 px-3 py-2 rounded-lg transition-colors {selectedUserId === user.id
								? 'bg-livnotePink text-black'
								: isAlreadyPublished
									? 'bg-osvauld-fieldActive text-textActive opacity-50 cursor-not-allowed'
									: 'bg-osvauld-fieldActive text-white hover:bg-osvauld-activeBorder'}"
							onclick={() => {
								if (!isAlreadyPublished) {
									selectedUserId = user.id;
								}
							}}
							disabled={isAlreadyPublished}
						>
							<div class="w-8 h-8 rounded-full {selectedUserId === user.id ? 'bg-black' : 'bg-livnotePink'} flex items-center justify-center {selectedUserId === user.id ? 'text-white' : 'text-black'} font-medium">
								{user.username.charAt(0).toUpperCase()}
							</div>
							<span class="flex-1 text-left flex items-center gap-2">
								{user.username}
								{#if isSovereignNode}
									<span class="text-xs px-2 py-0.5 bg-livnotePink/20 text-livnotePink rounded-full border border-livnotePink/40">Node</span>
								{/if}
							</span>
							{#if selectedUserId === user.id}
								<svg class="w-5 h-5" fill="currentColor" viewBox="0 0 20 20">
									<path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
								</svg>
							{/if}
						</button>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Generate Shareable Link Section -->
		{#if existingUsers.length > 0}
			<div class="mt-6 pt-4 border-t border-osvauld-borderColor">
				<h3 class="text-sm text-textActive mb-3">Generate Shareable Link:</h3>
				<p class="text-xs text-textActive mb-3">
					Select a published node to generate a shareable connection string for viewers.
				</p>

				<!-- Select Sovereign Node -->
				<div class="mb-3">
					<label class="block text-xs text-textActive mb-2">Select Node:</label>
					<select
						bind:value={selectedSovereignNodeId}
						class="w-full px-3 py-2 bg-osvauld-fieldActive text-white rounded-lg text-sm border border-osvauld-activeBorder focus:outline-none focus:ring-1 focus:ring-livnotePink"
					>
						<option value={null}>-- Select a node --</option>
						{#each existingUsers as user}
							<option value={user.id}>{user.username}</option>
						{/each}
					</select>
				</div>

				<button
					class="w-full px-4 py-2 bg-osvauld-fieldActive text-white rounded-lg text-sm hover:bg-osvauld-activeBorder transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
					onclick={handleGenerateLink}
					disabled={isGeneratingLink || !selectedSovereignNodeId}
				>
					{#if isGeneratingLink}
						<svg class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
							<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
							<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
						</svg>
						Requesting...
					{:else}
						<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
							<path d="M13.586 3.586a2 2 0 112.828 2.828l-.793.793-2.828-2.828.793-.793zM11.379 5.793L3 14.172V17h2.828l8.38-8.379-2.83-2.828z" />
						</svg>
						Generate Shareable Link
					{/if}
				</button>

				<!-- Display Generated Connection String -->
				{#if generatedConnectionString}
					<div class="mt-4 p-3 bg-osvauld-background rounded-lg border border-green-500/30">
						<div class="flex items-center justify-between mb-2">
							<span class="text-xs text-green-500 font-medium">✓ Connection String Generated</span>
							<button
								class="px-2 py-1 bg-osvauld-fieldActive text-white rounded text-xs hover:bg-osvauld-activeBorder transition-colors flex items-center gap-1"
								onclick={handleCopyConnectionString}
							>
								{#if isCopied}
									<svg class="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
										<path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
									</svg>
									Copied!
								{:else}
									<svg class="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
										<path d="M8 3a1 1 0 011-1h2a1 1 0 110 2H9a1 1 0 01-1-1z" />
										<path d="M6 3a2 2 0 00-2 2v11a2 2 0 002 2h8a2 2 0 002-2V5a2 2 0 00-2-2 3 3 0 01-3 3H9a3 3 0 01-3-3z" />
									</svg>
									Copy
								{/if}
							</button>
						</div>
						<div class="bg-osvauld-fieldActive p-2 rounded text-xs text-textActive break-all font-mono">
							{generatedConnectionString}
						</div>
					</div>
				{/if}
			</div>
		{/if}
		</div>

		<!-- Footer (fixed at bottom) -->
		<div class="flex justify-end gap-3 p-6 pt-4 border-t border-osvauld-borderColor">
			<button
				class="px-4 py-2 text-sm text-textActive hover:text-white transition-colors"
				onclick={onClose}
			>
				Cancel
			</button>
			<button
				class="px-6 py-2 bg-livnotePink text-black rounded-lg font-medium text-sm hover:bg-opacity-90 transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
				onclick={handlePublish}
				disabled={!selectedUserId || isPublishing}
			>
				{#if isPublishing}
					<svg class="animate-spin h-4 w-4" fill="none" viewBox="0 0 24 24">
						<circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
						<path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
					</svg>
					Publishing...
				{:else}
					Publish
				{/if}
			</button>
		</div>
	</div>
{/if}
