<script lang="ts">
	import { getLastModifiedDate } from "../../utils/helper";
	import { sendMessage } from "../../utils/helper";
	import ChatPreview from "./ChatPreview.svelte";
	import ChatListPanel from "../ui/ChatListPanel.svelte";
	import { dataState, uiState } from "../../state/";

	const selectChat = (chat: any) => {
		dataState.switchChat(chat.id);
	};

	const formatLastMessageTime = (timestamp: number | undefined) => {
		if (!timestamp) return "";
		const date = new Date(timestamp);
		const now = new Date();
		const diffInHours = (now.getTime() - date.getTime()) / (1000 * 60 * 60);
		
		if (diffInHours < 24) {
			return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
		} else {
			return date.toLocaleDateString();
		}
	};

	// Get participant names for a chat (excluding current user)
	const getChatParticipantNames = (chatId: string): string => {
		// First, try to get from the chat object itself
		const chat = dataState.getChatById(chatId);
		
		// If we have a coordinator loaded, use it to get more accurate names
		const coordinator = dataState.getChatCoordinator(chatId);
		if (coordinator) {
			const messages = coordinator.getMessages();
			if (messages && messages.length > 0) {
				const currentUserId = dataState.userDetails?.userId;
				const uniqueParticipants = new Set<string>();

				// Collect unique author names (excluding current user)
				messages.forEach(msg => {
					if (msg.authorId !== currentUserId && msg.authorName) {
						uniqueParticipants.add(msg.authorName);
					}
				});

				const participantArray = Array.from(uniqueParticipants);
				if (participantArray.length > 0) {
					return participantArray.join(", ");
				}
			}
		}

		// Fallback to stored participant name
		return chat?.participantName || "Chat";
	};

	// Get last message preview for a chat
	const getLastMessagePreview = (chatId: string): string => {
		const coordinator = dataState.getChatCoordinator(chatId);
		if (coordinator) {
			const messages = coordinator.getMessages();
			if (messages && messages.length > 0) {
				// Get the last message
				const lastMessage = messages[messages.length - 1];
				if (lastMessage?.content) {
					return lastMessage.content;
				}
			}
		}
		
		// Fallback to stored last message
		const chat = dataState.getChatById(chatId);
		return chat?.lastMessage || "No messages yet";
	};

	// Get unread count for a chat
	const getUnreadCount = (chatId: string): number => {
		const coordinator = dataState.getChatCoordinator(chatId);
		if (coordinator) {
			return coordinator.getUnreadCount();
		}
		
		// Fallback to stored unread count
		const chat = dataState.getChatById(chatId);
		return chat?.unreadCount || 0;
	};
</script>

<div class="h-full overflow-hidden flex flex-col">
	<ChatListPanel />
	<div class="flex-1 overflow-y-auto px-3 py-2">
		{#if dataState.isDataLoading}
			<div class="flex justify-center items-center h-full">
				<div class="text-osvauld-fieldText">Loading chats...</div>
			</div>
		{:else if dataState.filteredChats.length === 0}
			<div class="flex justify-center items-center h-full">
				<div class="text-osvauld-fieldText text-center px-4">
					No chats found. Start a new chat to get started.
				</div>
			</div>
		{:else}
			<div class="space-y-2">
				{#each dataState.filteredChats as chat (chat.id)}
					<div
						role="presentation"
						class="bg-osvauld-frameblack border rounded-lg overflow-hidden transition-all duration-200 cursor-pointer p-3 {
							dataState.currentChatId === chat.id 
								? 'border-livnotelavender bg-opacity-80' 
								: 'border-osvauld-borderColor hover:border-livnotelavender'
						}"
						onclick={() => selectChat(chat)}
					>
						<div class="flex justify-between items-start mb-1">
							<div class="flex items-center space-x-2 flex-1 min-w-0">
								<div class="w-2.5 h-2.5 rounded-full flex-shrink-0 {chat.isOnline ? 'bg-green-500' : 'bg-gray-500'}"></div>
								<h3 class="text-osvauld-fieldText font-medium truncate">
									{getChatParticipantNames(chat.id)}
								</h3>
								{#if getUnreadCount(chat.id) > 0}
									<span class="bg-livnotelavender text-white text-xs rounded-full px-1.5 py-0.5 min-w-[18px] text-center flex-shrink-0">
										{getUnreadCount(chat.id)}
									</span>
								{/if}
							</div>
							<div class="text-osvauld-fieldText opacity-60 text-xs flex-shrink-0 ml-2">
								{formatLastMessageTime(chat.lastMessageTime)}
							</div>
						</div>
						<div class="text-osvauld-fieldText opacity-70 text-sm truncate">
							{getLastMessagePreview(chat.id)}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
