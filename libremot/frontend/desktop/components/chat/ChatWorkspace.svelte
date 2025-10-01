<script lang="ts">
	import { dataState, uiState } from "../../state/";
	import { emit } from "@tauri-apps/api/event";
	import { onMount, tick } from "svelte";

	let messageInput = $state("");
	let isSending = $state(false);
	let messagesContainer: HTMLDivElement;

	const scrollToBottom = async () => {
		await tick();
		if (messagesContainer) {
			messagesContainer.scrollTop = messagesContainer.scrollHeight;
		}
	};

	// Watch for message changes and auto-scroll
	$effect(() => {
		// Access the messages to make this effect reactive
		dataState.currentChatMessages;
		scrollToBottom();
	});

	// Get all unique participants (excluding current user)
	const getParticipantNames = $derived.by(() => {
		if (!dataState.currentChatMessages || dataState.currentChatMessages.length === 0) {
			return dataState.getCurrentChat()?.participantName || "Chat";
		}

		const currentUserId = dataState.userDetails?.userId;
		const uniqueParticipants = new Set<string>();

		// Collect unique author names (excluding current user)
		dataState.currentChatMessages.forEach(msg => {
			if (msg.authorId !== currentUserId && msg.authorName) {
				uniqueParticipants.add(msg.authorName);
			}
		});

		// Convert to array and join with commas
		const participantArray = Array.from(uniqueParticipants);
		return participantArray.length > 0 ? participantArray.join(", ") : "Chat";
	});

	const sendChatMessage = async () => {
		if (!messageInput.trim() || !dataState.getCurrentChatId()) return;
		
		isSending = true;
		try {
			const content = messageInput.trim();
			messageInput = ""; // Clear input immediately
			
			// Use the dataState method which handles coordinator
			await dataState.sendChatMessage(content);
			
			// Scroll to bottom after sending
			scrollToBottom();
		} catch (error) {
			console.error("Error sending message:", error);
			uiState.showToast("Failed to send message", false);
		} finally {
			isSending = false;
		}
	};

	const handleKeyPress = (event: KeyboardEvent) => {
		if (event.key === "Enter" && !event.shiftKey) {
			event.preventDefault();
			sendChatMessage();
		}
	};

	const formatMessageTime = (timestamp: number) => {
		return new Date(timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
	};
</script>

<div class="flex flex-col h-full w-full bg-osvauld-ninjablack">
	<!-- Chat Header -->
	<div class="flex items-center p-4 border-b border-osvauld-borderColor bg-osvauld-frameblack w-full">
		<div class="flex items-center space-x-3">
			<!-- Avatar Circle with First Letter -->
			<div class="w-10 h-10 rounded-full bg-livnotelavender flex items-center justify-center text-white font-semibold text-lg">
				{getParticipantNames?.[0]?.toUpperCase() || "?"}
			</div>
			<div class="flex flex-col">
				<h2 class="text-osvauld-fieldText font-medium text-base">
					{getParticipantNames}
				</h2>
				<div class="flex items-center space-x-1.5">
					<div class="w-2 h-2 rounded-full {dataState.getCurrentChat()?.isOnline ? 'bg-green-500' : 'bg-gray-500'}"></div>
					<span class="text-osvauld-fieldText opacity-60 text-xs">
						{dataState.getCurrentChat()?.isOnline ? 'Online' : 'Offline'}
					</span>
				</div>
			</div>
		</div>
	</div>

	<!-- Messages Area -->
	<div bind:this={messagesContainer} class="flex-1 overflow-y-auto p-6 space-y-3 bg-osvauld-ninjablack w-full">
		{#each dataState.currentChatMessages as message (message.id)}
			<div class="flex flex-col {message.authorId === dataState.userDetails?.userId ? 'items-end' : 'items-start'} w-full">
				<!-- Sender Name -->
				<div class="text-xs font-medium mb-1 px-1 {
					message.authorId === dataState.userDetails?.userId 
						? 'text-livnotelavender' 
						: 'text-osvauld-fieldText opacity-70'
				}">
					{message.authorName}
				</div>
				<!-- Message Bubble -->
				<div class="max-w-md px-3 py-2 rounded-lg {
					message.authorId === dataState.userDetails?.userId 
						? 'bg-livnotelavender text-white rounded-br-none' 
						: 'bg-osvauld-frameblack text-osvauld-fieldText border border-osvauld-borderColor rounded-bl-none'
				}">
					<div class="text-sm break-words">
						{message.content}
					</div>
					<div class="text-xs opacity-60 mt-1 text-right {
						message.authorId === dataState.userDetails?.userId 
							? 'text-white' 
							: 'text-osvauld-fieldText'
					}">
						{formatMessageTime(message.timestamp)}
					</div>
				</div>
			</div>
		{/each}
	</div>

	<!-- Message Input -->
	<div class="p-4 border-t border-osvauld-borderColor bg-osvauld-frameblack w-full">
		<div class="flex items-end space-x-3 w-full">
			<textarea
				bind:value={messageInput}
				onkeypress={handleKeyPress}
				placeholder="Type a message..."
				disabled={isSending}
				class="flex-1 bg-osvauld-ninjablack border border-osvauld-borderColor rounded-xl px-4 py-3 text-sm text-osvauld-fieldText placeholder-osvauld-fieldText placeholder-opacity-60 focus:outline-none focus:border-livnotelavender resize-none max-h-32"
				rows="1"
			></textarea>
			<button
				onclick={sendChatMessage}
				disabled={!messageInput.trim() || isSending}
				class="bg-livnotelavender text-white p-3 rounded-full hover:bg-opacity-80 transition-colors duration-200 disabled:opacity-50 disabled:cursor-not-allowed flex-shrink-0"
				title="Send message"
			>
				<svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8" />
				</svg>
			</button>
		</div>
	</div>
</div>
