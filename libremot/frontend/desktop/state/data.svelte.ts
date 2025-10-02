import { sendMessage } from "../utils/helper";
import { uiState } from './ui.svelte';
import { listen, emit } from "@tauri-apps/api/event";
import { StoreService } from './storeService';
import { ChatCoordinator, type ChatMessage as CoordinatorChatMessage } from "../components/chat/chatCoordinator";
import * as Y from 'yjs';
export interface Chat {
  id: string;
  title: string;
  lastMessage?: string;
  lastMessageTime?: number;
  unreadCount: number;
  isOnline: boolean;
  participantId: string;
  participantName: string;
}

export interface ChatMessage {
  id: string;
  authorId: string;
  authorName: string;
  content: string;
  timestamp: number;
  type: 'text' | 'image';
  imageId?: string;
}

export interface UserDetails {
  userId: string,
  deviceId: string,
  username: string,
  publicKey: string,
  deviceKey: string,
}

// Data State class
class DataState {
  chats = $state<Chat[]>([]);
  currentChat = $state<Chat | null>(null);
  currentChatMessages = $state<ChatMessage[]>([]);
  private chatCoordinators = $state<Map<string, ChatCoordinator>>(new Map());
  isDataLoading = $state<boolean>(false);
  userDetails = $state<UserDetails | null>(null)
  currentChatId = $state<string | null>(null);
  private _unlisteners: Array<() => void> = [];
  private _messageObservers: Map<string, () => void> = new Map();
  clientId: number = 0;
  searchResults = $state<string[]>([]);
  isSearchActive = $state<boolean>(false);

  getChatCoordinator(chatId: string): ChatCoordinator | null {
    return this.chatCoordinators.get(chatId) || null;
  }
  
  getCurrentChatId(): string | null {
    return this.currentChatId;
  }
  
  setCurrentChatId(chatId: string | null) {
    this.currentChatId = chatId;
  }

  setCurrentChat(chat: Chat | null) {
    this.currentChat = chat;
  }

  getCurrentChat(): Chat | null {
    return this.currentChat;
  }
  // Filtered chats for search
  filteredChats = $derived.by(() => {
    if (!this.isSearchActive || this.searchResults.length === 0) {
      return this.chats;
    }

    // Apply search filter to chats
    return this.chats.filter(chat => this.searchResults.includes(chat.id));
  });
  setSearchResults(chatIds: string[]) {
    this.searchResults = chatIds;
    this.isSearchActive = chatIds.length > 0;
  }

  // Method to clear search
  clearSearch() {
    this.searchResults = [];
    this.isSearchActive = false;
  }
  
  /**
   * Get a chat by its ID from the cached chats
   * @param id The ID of the chat to find
   * @returns The chat object if found, null otherwise
   */
  getChatById(id: string): Chat | null {
    const chat = this.chats.find(chat => chat.id === id);
    return chat || null;
  }

  async fetchChats() {
    try {
      // Get the default folder first
      const defaultFolder = await this.getDefaultFolder();
      if (!defaultFolder) {
        console.warn("No default folder found for fetching chats");
        this.chats = [];
        return;
      }

      const resp = await sendMessage("getCredentialsForFolder", { folderId: defaultFolder.id });
      const chatResources: Chat[] = resp.map((item: any) => ({
        id: item.id || "",
        title: item.title || "Untitled Chat",
        lastMessage: item.lastMessage || "",
        lastMessageTime: item.lastMessageTime || 0,
        unreadCount: item.unreadCount || 0,
        isOnline: item.isOnline || false,
        participantId: item.participantId || "",
        participantName: item.participantName || "Unknown"
      }));
      this.chats = chatResources;
    } catch (error) {
      console.error("Error fetching chats:", error);
    }
  }

  async fetchAllChats(selectedChatId?: string) {
    this.isDataLoading = true;
    this.chats = [];
    try {
      const response = await sendMessage("emitAllResources", selectedChatId);
      if (response) {
        this.setCurrentChat(response);
      }
    } catch (error) {
      console.error("Error fetching chats:", error);
      this.chats = [];
    } finally {
      this.isDataLoading = false;
    }
  }
  async addChat(participantId: string, participantName: string) {
    try {
      uiState.setNoteFetching(true);
      uiState.setEditorLoading(false);

      // Get the default folder for new chats
      const defaultFolder = await this.getDefaultFolder();
      if (!defaultFolder) {
        uiState.showToast("No default folder found", false);
        uiState.setNoteFetching(false);
        return;
      }

      // Create the chat content with YJS document structure
      const chatContent = this.createEmptyChatContent();
      const chat = await sendMessage("addCredential", {
        resourcePayload: JSON.stringify(chatContent),
        resourceType: "chat",
        folderId: defaultFolder.id
      });

      // Share the chat resource with the selected user
      await this.shareChatWithUser(chat.id, participantId, participantName);

      this.setCurrentChat(chat);
      this.setCurrentChatId(chat.id);
      
      // Load empty chat into coordinator
      if (chat.data) {
        await this.loadChat(chat.id, chat.data);
      }
      
      uiState.setNoteFetching(false);
      uiState.toggleNoteViewLayout(true);
      StoreService.setCurrentNoteId(chat.id);
      
      // Emit "note-change" to notify backend of active chat
      emit("note-change", chat.id).catch(error => {
        console.error("Error updating current chat:", error);
      });

      uiState.showToast(`Chat with ${participantName} created`, true);
    } catch (error) {
      console.error("Error creating chat:", error);
      uiState.showToast("Failed to create chat", false);
      uiState.setNoteFetching(false);
    }
  }

  async shareChatWithUser(chatId: string, userId: string, username: string) {
    try {
      // Define permissions for chat resource
      const permissions = [
        [`livnote:resource:${chatId}`, "crud/read"],
        [`livnote:resource:${chatId}`, "crud/update"],
        [`livnote:resource:${chatId}`, "ucan/share"]
      ];

      await sendMessage("shareResource", {
        userId: userId,
        resourceId: chatId,
        permissions: permissions
      });

      console.log(`Chat ${chatId} shared with user ${username}`);
    } catch (error) {
      console.error("Error sharing chat resource:", error);
      throw error;
    }
  }

  async showUserSelectionModal() {
    try {
      // Get available users for chat
      const availableUsers = await this.getAvailableUsers();
      if (availableUsers.length === 0) {
        uiState.showToast("No users available for chat", false);
        return;
      }

      // Show user selection modal (you'll need to implement this in UI state)
      uiState.showUserSelectionModal(availableUsers);
    } catch (error) {
      console.error("Error fetching available users:", error);
      uiState.showToast("Failed to load users", false);
    }
  }

  async getAvailableUsers() {
    try {
      const response = await sendMessage("getKnownUsers");
      if (response && Array.isArray(response)) {
        return response.map((user: any) => ({
          id: user.id,
          username: user.username,
          publicKey: user.public_key,
          isOnline: user.is_online || false
        }));
      }
      return [];
    } catch (error) {
      console.error("Error fetching available users:", error);
      return [];
    }
  }

  async createChatWithUser(userId: string, username: string) {
    // Close the user selection modal
    uiState.hideUserSelectionModal();
    
    // Create the chat with the selected user
    await this.addChat(userId, username);
  }

  async getDefaultFolder() {
    try {
      const response = await sendMessage("getFolder");
      if (response && Array.isArray(response)) {
        const defaultFolder = response.find((folder: any) => folder.default_folder === true);
        return defaultFolder || null;
      }
      return null;
    } catch (error) {
      console.error("Error fetching default folder:", error);
      return null;
    }
  }

  createEmptyChatContent() {
    // Create fresh YJS documents
    const chatDoc = new Y.Doc();
    const imageDoc = new Y.Doc();
    
    // Initialize the shared types (even though empty, this creates valid YJS state)
    chatDoc.getMap('messages');
    imageDoc.getMap('images');
    
    // Encode as V2 updates (to match backend expectation)
    const chatState = Y.encodeStateAsUpdateV2(chatDoc);
    const imageState = Y.encodeStateAsUpdateV2(imageDoc);
    
    return {
      chat: Array.from(chatState),           // Valid YJS V2 state for empty Y.Map
      image_state: Array.from(imageState)    // Valid YJS V2 state for empty Y.Map
    };
  }

  getChatTitle(): string {
    return this.getCurrentChat()?.title || "Untitled Chat";
  }

  async switchChat(chatId: string | null) {
    // Emit "note-change" to notify backend of active chat (backend uses same listener)
    emit("note-change", chatId).catch(error => {
      uiState.clearAllLoadingStates();
      console.error("Error updating current chat:", error);
    });
    if (chatId) {
      uiState.setNoteFetching(true);
      uiState.setEditorLoading(false);
      // No longer need to toggle view layout - we have split view now
      
      const chat = await sendMessage("getCredential", { resourceId: chatId });
      this.setCurrentChat(chat);
      this.setCurrentChatId(chatId);
      
      // Load chat content into coordinator
      if (chat.data) {
        await this.loadChat(chatId, chat.data);
        
        // Mark all messages as read when opening the chat
        const coordinator = this.getChatCoordinator(chatId);
        if (coordinator) {
          coordinator.markAllMessagesAsRead();
        }
      }
      
      uiState.setNoteFetching(false);
      StoreService.setCurrentNoteId(chatId);
    } else {
      dataState.clearCurrentChat();
    }
  }

  clearCurrentChat() {
    this.setCurrentChatId(null);
    this.setCurrentChat(null);
    this.currentChatMessages = [];
    StoreService.setCurrentNoteId(null);
  }
  private createChatCoordinator(chatId: string) {
    if (this.chatCoordinators.has(chatId)) {
      const existing = this.chatCoordinators.get(chatId);
      existing?.destroy();
      // Cleanup observer
      const observer = this._messageObservers.get(chatId);
      if (observer) {
        observer();
        this._messageObservers.delete(chatId);
      }
    }

    if (!this.userDetails) {
      throw new Error("User details not available for coordinator creation");
    }

    const userInfo = {
      name: this.userDetails.username,
      color: this.generateUserColor(),
      id: this.clientId,
      userId: this.userDetails.userId,
    };
    
    const coordinator = new ChatCoordinator({
      userInfo,
      onChatUpdate: async (update) => {
        if (!this.getCurrentChatId()) return;
        await emit("sync-update", {
          update: Array.from(update),
          clientID: this.clientId,
          resource_id: this.getCurrentChatId(),
          doc_type: "chat",
        });
      },
      onImageUpdate: async (update) => {
        if (!this.getCurrentChatId()) return;
        await emit("sync-update", {
          update: Array.from(update),
          clientID: this.clientId,
          resource_id: this.getCurrentChatId(),
          doc_type: "image_state",
        });
      },
    });
    
    // Subscribe to message changes
    const unobserve = coordinator.onMessagesChange((messages) => {
      if (this.currentChatId === chatId) {
        this.currentChatMessages = messages;
      }
    });
    
    this._messageObservers.set(chatId, unobserve);
    this.chatCoordinators.set(chatId, coordinator);
    
    return coordinator;
  }

  async initializeState() {
    this.isDataLoading = true;

    // Clear any existing state and event listeners first
    this.clearAllState();

    const savedChatId = await StoreService.getCurrentNoteId();
    await Promise.all([
      this.fetchChats(),
      this.getUserDetails(),
      this.setupReactiveUpdates()
    ]);
    await this.restoreSavedSelections();
    this.isDataLoading = false;
    if (savedChatId) {
      this.fetchAllChats(savedChatId);
    } else {
      this.fetchAllChats();
    }
  }

  // Add a method to clear all state when logging out
  clearAllState() {
    // Clear chats state
    this.chats = [];
    this.currentChatId = null;
    this.currentChat = null;
    this.currentChatMessages = [];

    // Clean up event listeners
    this.cleanupReactiveUpdates();

    // Clean up coordinators
    this.chatCoordinators.forEach(coordinator => {
      coordinator.destroy();
    });
    this.chatCoordinators.clear();
  }
  private generateUserColor(): string {
    const colors = [
      "#FF5630",
      "#FFAB00",
      "#36B37E",
      "#00B8D9",
      "#6554C0",
      "#FF7452",
    ];
    return colors[Math.floor(Math.random() * colors.length)];
  }

  async getUserDetails() {
    this.userDetails = await sendMessage('getUserDetails');
    this.clientId = this.getClientId();
  }

  getClientId(): number {
    if (!this.userDetails) {
      throw new Error("User details not available");
    }
    const decoded = atob(this.userDetails.deviceId);
    const bytes = new Uint8Array(decoded.length);
    for (let i = 0; i < decoded.length; i++) {
      bytes[i] = decoded.charCodeAt(i);
    }
    const hex = Array.from(bytes.slice(0, 4))
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');

    return parseInt(hex, 16);
  }

  async restoreSavedSelections() {
    // No vault selection to restore for chats
    return;
  }
  async handleAwarenessUpdates(event: any) {
    try {
      const { resource_id, connection_id, updates, client_id } = event.payload;
      // For now, awareness is not implemented in ChatCoordinator
      // Can be added later for typing indicators, online status, etc.
      console.log("Awareness update received for chat:", resource_id);
    } catch (error) {
      console.error("Error handling awareness-updates:", error);
    }
  }

  async handleLiveUpdates(event: any) {
    try {
      const { resource_id, updates, client_id, doc_type } = event.payload;
      
      // Get or create coordinator for this chat
      // We need to apply updates even if chat is not currently open
      let coordinator = this.getChatCoordinator(resource_id);
      
      if (!coordinator) {
        // If coordinator doesn't exist yet, we need to create it
        // This can happen when we receive a message before opening the chat
        console.log("Creating coordinator for incoming message:", resource_id);
        
        // Find the chat in our chats list
        const chat = this.chats.find(c => c.id === resource_id);
        if (!chat) {
          console.warn("Received update for unknown chat:", resource_id);
          return;
        }
        
        // Create coordinator (this will set up the Y.Doc)
        coordinator = this.createChatCoordinator(resource_id);
      }
      
      const updatesArray = new Uint8Array(updates);
      const senderId = parseInt(client_id, 10);
      
      // Backend sends "chat" or "image_state" directly (no mapping needed)
      const coordinatorDocType = doc_type as 'chat' | 'image_state';
      
      // Apply the update
      console.log(`Applying update to chat ${resource_id}:`, {
        updateSize: updatesArray.length,
        senderId,
        docType: coordinatorDocType,
        isCurrentChat: this.currentChatId === resource_id
      });
      
      coordinator.applyRemoteUpdate(updatesArray, senderId, coordinatorDocType);
      
      // Check message count after applying
      const messages = coordinator.getMessages();
      console.log(`After applying update, chat ${resource_id} has ${messages.length} messages`);
      
      // If this is NOT the currently open chat, we should update the preview
      // The coordinator will handle applying to the Y.Doc
      // The preview update will come via the "chat-preview-update" event
      
      console.log(`Applied update for chat ${resource_id} (current: ${this.currentChatId === resource_id})`);
    } catch (error) {
      console.error("Error handling live-updates:", error);
    }
  }

  async handleChatPreviewUpdate(event: any) {
    try {
      const { resource_id, last_message, last_message_time, unread_count, participants } = event.payload;
      
      console.log(`Chat preview update for ${resource_id}:`, {
        last_message,
        unread_count,
        last_message_time
      });
      
      // Find and update the chat in the list
      const chatIndex = this.chats.findIndex(c => c.id === resource_id);
      if (chatIndex >= 0) {
        const chat = this.chats[chatIndex];
        
        // Update chat preview data
        this.chats[chatIndex] = {
          ...chat,
          lastMessage: last_message,
          lastMessageTime: last_message_time,
          unreadCount: unread_count,
          // Update participants if provided
          ...(participants && { participants })
        };
        
        // Re-sort chats by last message time (most recent first)
        this.chats = this.chats.sort((a, b) => {
          const timeA = a.lastMessageTime || 0;
          const timeB = b.lastMessageTime || 0;
          return timeB - timeA;
        });
        
        console.log(`Updated chat preview for ${resource_id}, unread: ${unread_count}`);
      } else {
        console.warn(`Chat ${resource_id} not found in list for preview update`);
      }
    } catch (error) {
      console.error("Error handling chat-preview-update:", error);
    }
  }

  async handleResourceAddedNotification(event: any) {
    console.log(event.payload);
    let message = `${event.payload.username} started a chat`
    uiState.showToast(message, true);
  }

  async setupReactiveUpdates() {
    this._unlisteners = [];

    const resourceAddedUnlisten = await listen("resource-added", this.handleResourceAdded.bind(this));
    const resourceUpdateUnlisten = await listen("resource-update", this.handleResourceUpdate.bind(this));
    const documentUpdatesUnlisten = await listen("document-updates", this.handleDocumentUpdates.bind(this));
    const awarenessUpdatesUnlisten = await listen("awareness-updates", this.handleAwarenessUpdates.bind(this));
    const liveUpdatesUnlisten = await listen("live-updates", this.handleLiveUpdates.bind(this));
    const chatPreviewUpdateUnlisten = await listen("chat-preview-update", this.handleChatPreviewUpdate.bind(this));
    const resourceAddedNotification = await listen(
      "resource-added-notification",
      this.handleResourceAddedNotification.bind(this));

    this._unlisteners.push(
      resourceAddedUnlisten,
      resourceUpdateUnlisten,
      documentUpdatesUnlisten,
      awarenessUpdatesUnlisten,
      liveUpdatesUnlisten,
      chatPreviewUpdateUnlisten,
      resourceAddedNotification,
    );
  }

  cleanupReactiveUpdates() {
    if (this._unlisteners) {
      for (const unlisten of this._unlisteners) {
        unlisten();
      }
      this._unlisteners = [];
    }
  }

  handleResourceAdded(event: any) {
    const chatPreview = event.payload;
    // Check if chat already exists to prevent duplicates
    const existingChat = this.chats.find(chat => chat.id === chatPreview.id);
    if (!existingChat) {
      this.chats = [...this.chats, chatPreview];
    }
  }

  handleResourceUpdate(event: any) {
    const updatedChatPreview = event.payload;
    const chatIndex = this.chats.findIndex(chat => chat.id === updatedChatPreview.id);

    if (chatIndex !== -1) {
      this.chats = [
        ...this.chats.slice(0, chatIndex),
        updatedChatPreview,
        ...this.chats.slice(chatIndex + 1)
      ];
    } else {
      // Only add if it doesn't already exist
      const existingChat = this.chats.find(chat => chat.id === updatedChatPreview.id);
      if (!existingChat) {
        this.chats = [...this.chats, updatedChatPreview];
      }
    }
  }

  async saveChat(chatId: string) {
    const coordinator = this.getChatCoordinator(chatId);
    if (!coordinator) {
      throw new Error("Coordinator not available");
    }
    const chatContent = coordinator.saveChat();
    let stateVectors = await coordinator.getStateVectors();
    await sendMessage("updateCredential", {
      id: chatId,
      data: JSON.stringify(chatContent),
    });
    emit("resource-update-complete", { id: chatId, state_vectors: stateVectors });
    uiState.setNoteSaved(true);

    setTimeout(() => {
      uiState.setNoteSaved(false);
    }, 1500);
  }

  async sendChatMessage(message: string): Promise<void> {
    const chatId = this.getCurrentChatId();
    if (!chatId) {
      console.error("No active chat");
      return;
    }

    const coordinator = this.getChatCoordinator(chatId);
    if (!coordinator) {
      console.error("No coordinator for chat:", chatId);
      return;
    }

    console.log("Sending message:", message);
    // Add message to Y.Map - this will trigger the observer and update currentChatMessages
    const addedMessage = coordinator.addMessage(message, 'text');
    console.log("Message added to coordinator:", addedMessage);
    
    // Save to backend
    await this.saveChat(chatId);
  }

  async handleDocumentUpdates(event: any) {
    try {
      const { resource_id, updates } = event.payload;
      if (resource_id == this.currentChatId) {
        const updatesJson = JSON.parse(updates)
        const imageUpdates = updatesJson.image_state?.updates;
        const chatUpdates = updatesJson.chat?.updates;
        const coordinator = this.getChatCoordinator(resource_id);
        
        if (imageUpdates) {
          const imageUpdateArray = new Uint8Array(imageUpdates);
          coordinator?.applyRemoteUpdate(imageUpdateArray, updatesJson.client_id, "image_state");
        }
        
        if (chatUpdates) {
          const chatUpdateArray = new Uint8Array(chatUpdates);
          coordinator?.applyRemoteUpdate(chatUpdateArray, updatesJson.client_id, "chat");
        }
      }
    } catch (error) {
      console.error("Error handling document-updates:", error);
    }
  }

  // Load chat and initialize coordinator
  async loadChat(chatId: string, chatContent: { chat: number[], image_state: number[] }): Promise<void> {
    console.log("Loading chat:", chatId, "with content:", chatContent);
    const coordinator = this.createChatCoordinator(chatId);
    await coordinator.loadChat(chatContent);
    
    // Update current messages
    const messages = coordinator.getMessages();
    console.log("Loaded messages:", messages);
    this.currentChatMessages = messages;
  }

}

export const dataState = new DataState();

