import * as Y from 'yjs';

export interface ChatMessage {
  id: string;
  authorId: string;
  authorName: string;
  content: string;
  timestamp: number;
  type: 'text' | 'image';
  imageId?: string;  // Reference to image in image_state
  readBy?: string[];  // Array of user IDs who have read this message
}

export interface UserInfo {
  id: number;
  userId: string;
  name: string;
  color: string;
}

export interface ChatCoordinatorConfig {
  userInfo: UserInfo;
  onChatUpdate?: (update: Uint8Array) => void;
  onImageUpdate?: (update: Uint8Array) => void;
  onAwarenessUpdate?: (changes: Uint8Array) => void;
}

/**
 * Coordinates YJS documents for chat messages and images
 * Simpler than NotesCoordinator - no ProseMirror, just Y.Map and Y.Array
 */
export class ChatCoordinator {
  private chatDoc: Y.Doc;
  private imageDoc: Y.Doc;
  private awareness: any;
  private messages: Y.Map<ChatMessage>;
  private images: Y.Map<any>;
  private userInfo: UserInfo;
  private config: ChatCoordinatorConfig;

  constructor(config: ChatCoordinatorConfig) {
    this.config = config;
    this.userInfo = config.userInfo;
    
    // Initialize YJS documents
    this.chatDoc = new Y.Doc();
    this.imageDoc = new Y.Doc();
    
    // Get shared types
    this.messages = this.chatDoc.getMap('messages');
    this.images = this.imageDoc.getMap('images');
    
    // Setup update handlers
    this.setupUpdateHandlers();
  }

  private setupUpdateHandlers(): void {
    // Chat document updates (use V2 encoding to match backend)
    this.chatDoc.on('updateV2', (update: Uint8Array, origin: any) => {
      if (origin !== 'remote' && origin !== 'loading' && this.config.onChatUpdate) {
        this.config.onChatUpdate(update);
      }
    });

    // Image document updates (use V2 encoding to match backend)
    this.imageDoc.on('updateV2', (update: Uint8Array, origin: any) => {
      if (origin !== 'remote' && origin !== 'loading' && this.config.onImageUpdate) {
        this.config.onImageUpdate(update);
      }
    });
  }

  /**
   * Load existing chat content (use V2 decoding to match V2 encoding)
   */
  async loadChat(chatContent: { chat: number[], image_state: number[] }): Promise<void> {
    console.log('Loading chat content:', { 
      chatSize: chatContent.chat?.length, 
      imageSize: chatContent.image_state?.length 
    });
    
    if (chatContent.chat && chatContent.chat.length > 0) {
      // Use V2 decoding for V2 encoded data
      Y.applyUpdateV2(this.chatDoc, new Uint8Array(chatContent.chat), 'loading');
      console.log('Chat doc loaded, messages in Y.Map:', this.messages.size);
    }
    
    if (chatContent.image_state && chatContent.image_state.length > 0) {
      // Use V2 decoding for V2 encoded data
      Y.applyUpdateV2(this.imageDoc, new Uint8Array(chatContent.image_state), 'loading');
      console.log('Image doc loaded, images in Y.Map:', this.images.size);
    }
  }

  /**
   * Add a new message to the chat
   */
  addMessage(content: string, type: 'text' | 'image' = 'text', imageId?: string): ChatMessage {
    const message: ChatMessage = {
      id: this.generateMessageId(),
      authorId: this.userInfo.userId,
      authorName: this.userInfo.name,
      content,
      timestamp: Date.now(),
      type,
      imageId,
      readBy: [this.userInfo.userId] // Author has read their own message
    };

    // Add message to Y.Map with message ID as key
    this.messages.set(message.id, message);
    
    return message;
  }

  /**
   * Get all messages sorted by timestamp
   */
  getMessages(): ChatMessage[] {
    const messageArray: ChatMessage[] = [];
    this.messages.forEach((message) => {
      messageArray.push(message);
    });
    
    // Sort by timestamp
    return messageArray.sort((a, b) => a.timestamp - b.timestamp);
  }

  /**
   * Delete a message
   */
  deleteMessage(messageId: string): void {
    this.messages.delete(messageId);
  }

  /**
   * Add image metadata
   */
  addImage(imageId: string, metadata: {
    url: string;
    size: number;
    mimeType: string;
    thumbnailUrl?: string;
  }): void {
    this.images.set(imageId, {
      id: imageId,
      uploadedBy: this.userInfo.userId,
      uploadedAt: Date.now(),
      ...metadata
    });
  }

  /**
   * Get image metadata
   */
  getImage(imageId: string): any {
    return this.images.get(imageId);
  }

  /**
   * Get all images
   */
  getAllImages(): Map<string, any> {
    const imageMap = new Map();
    this.images.forEach((image, key) => {
      imageMap.set(key, image);
    });
    return imageMap;
  }

  /**
   * Apply remote update from peer (use V2 decoding)
   */
  applyRemoteUpdate(update: Uint8Array | number[], sender: number, docType: 'chat' | 'image_state' = 'chat'): void {
    if (sender === this.userInfo.id) return;

    const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);
    
    if (docType === 'chat') {
      // Use V2 decoding for V2 encoded updates
      Y.applyUpdateV2(this.chatDoc, updateArray, 'remote');
    } else {
      Y.applyUpdateV2(this.imageDoc, updateArray, 'remote');
    }
  }

  /**
   * Save current chat state (use V2 encoding to match backend)
   */
  saveChat(): { chat: number[], image_state: number[] } {
    return {
      chat: Array.from(Y.encodeStateAsUpdateV2(this.chatDoc)),
      image_state: Array.from(Y.encodeStateAsUpdateV2(this.imageDoc))
    };
  }

  /**
   * Get state vectors for sync
   */
  async getStateVectors(): Promise<Record<string, any>> {
    const chatStateVector = Y.encodeStateVector(this.chatDoc);
    const imageStateVector = Y.encodeStateVector(this.imageDoc);
    
    return {
      chat: {
        updates: [],
        state_vector: Array.from(chatStateVector)
      },
      image_state: {
        updates: [],
        state_vector: Array.from(imageStateVector)
      }
    };
  }

  /**
   * Generate updates for peer based on their state vector
   */
  async generateUpdatesForPeer(peerStateVectors: string): Promise<string> {
    const stateVectors = JSON.parse(peerStateVectors);
    const result: Record<string, any> = {};

    // Generate chat updates
    if (stateVectors.chat && stateVectors.chat.state_vector) {
      const peerChatVector = new Uint8Array(stateVectors.chat.state_vector);
      const chatUpdate = Y.encodeStateAsUpdate(this.chatDoc, peerChatVector);
      const currentChatVector = Y.encodeStateVector(this.chatDoc);
      
      result.chat = {
        updates: Array.from(chatUpdate),
        state_vector: Array.from(currentChatVector)
      };
    }

    // Generate image updates
    if (stateVectors.image_state && stateVectors.image_state.state_vector) {
      const peerImageVector = new Uint8Array(stateVectors.image_state.state_vector);
      const imageUpdate = Y.encodeStateAsUpdate(this.imageDoc, peerImageVector);
      const currentImageVector = Y.encodeStateVector(this.imageDoc);
      
      result.image_state = {
        updates: Array.from(imageUpdate),
        state_vector: Array.from(currentImageVector)
      };
    }

    return JSON.stringify(result);
  }

  /**
   * Apply updates and generate diff
   */
  async applyUpdatesAndGenerateDiff(remoteUpdates: string): Promise<string> {
    const updates = JSON.parse(remoteUpdates);
    const result: Record<string, any> = {};

    // Apply and generate chat diff
    if (updates.chat && updates.chat.updates && updates.chat.updates.length > 0) {
      const chatUpdate = new Uint8Array(updates.chat.updates);
      Y.applyUpdate(this.chatDoc, chatUpdate, 'sync');
    }
    
    if (updates.chat && updates.chat.state_vector) {
      const peerChatVector = new Uint8Array(updates.chat.state_vector);
      const chatDiff = Y.encodeStateAsUpdate(this.chatDoc, peerChatVector);
      const currentChatVector = Y.encodeStateVector(this.chatDoc);
      
      result.chat = {
        updates: Array.from(chatDiff),
        state_vector: Array.from(currentChatVector)
      };
    }

    // Apply and generate image diff
    if (updates.image_state && updates.image_state.updates && updates.image_state.updates.length > 0) {
      const imageUpdate = new Uint8Array(updates.image_state.updates);
      Y.applyUpdate(this.imageDoc, imageUpdate, 'sync');
    }
    
    if (updates.image_state && updates.image_state.state_vector) {
      const peerImageVector = new Uint8Array(updates.image_state.state_vector);
      const imageDiff = Y.encodeStateAsUpdate(this.imageDoc, peerImageVector);
      const currentImageVector = Y.encodeStateVector(this.imageDoc);
      
      result.image_state = {
        updates: Array.from(imageDiff),
        state_vector: Array.from(currentImageVector)
      };
    }

    return JSON.stringify(result);
  }

  /**
   * Apply peer updates
   */
  async applyPeerUpdates(updates: string): Promise<void> {
    const parsedUpdates = JSON.parse(updates);

    if (parsedUpdates.chat && parsedUpdates.chat.updates && parsedUpdates.chat.updates.length > 0) {
      const chatUpdate = new Uint8Array(parsedUpdates.chat.updates);
      Y.applyUpdate(this.chatDoc, chatUpdate, 'sync');
    }

    if (parsedUpdates.image_state && parsedUpdates.image_state.updates && parsedUpdates.image_state.updates.length > 0) {
      const imageUpdate = new Uint8Array(parsedUpdates.image_state.updates);
      Y.applyUpdate(this.imageDoc, imageUpdate, 'sync');
    }
  }

  /**
   * Subscribe to message changes
   */
  onMessagesChange(callback: (messages: ChatMessage[]) => void): () => void {
    const observer = () => {
      callback(this.getMessages());
    };
    
    this.messages.observe(observer);
    
    // Return unsubscribe function
    return () => {
      this.messages.unobserve(observer);
    };
  }

  /**
   * Subscribe to image changes
   */
  onImagesChange(callback: (images: Map<string, any>) => void): () => void {
    const observer = () => {
      callback(this.getAllImages());
    };
    
    this.images.observe(observer);
    
    // Return unsubscribe function
    return () => {
      this.images.unobserve(observer);
    };
  }

  /**
   * Clean up resources
   */
  destroy(): void {
    this.chatDoc.destroy();
    this.imageDoc.destroy();
  }

  /**
   * Mark a message as read by current user
   */
  markMessageAsRead(messageId: string): void {
    const message = this.messages.get(messageId);
    if (!message) return;

    // Initialize readBy if it doesn't exist
    if (!message.readBy) {
      message.readBy = [];
    }

    // Add current user to readBy if not already present
    if (!message.readBy.includes(this.userInfo.userId)) {
      message.readBy.push(this.userInfo.userId);
      this.messages.set(messageId, message);
    }
  }

  /**
   * Mark all messages as read by current user
   */
  markAllMessagesAsRead(): void {
    const messages = this.getMessages();
    messages.forEach(message => {
      if (message.authorId !== this.userInfo.userId) {
        this.markMessageAsRead(message.id);
      }
    });
  }

  /**
   * Get unread message count for current user
   */
  getUnreadCount(): number {
    const messages = this.getMessages();
    return messages.filter(msg => 
      msg.authorId !== this.userInfo.userId && 
      (!msg.readBy || !msg.readBy.includes(this.userInfo.userId))
    ).length;
  }

  /**
   * Generate unique message ID
   */
  private generateMessageId(): string {
    return `${this.userInfo.userId}-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
  }
}
