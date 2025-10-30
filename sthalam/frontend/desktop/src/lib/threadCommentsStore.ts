import * as Y from "yjs";

export interface Comment {
  id: string;
  author: string;
  userId?: string;
  content: string;
  timestamp: number;
}

/**
 * Manages thread comments using YJS
 * Follows livnote's CommentsStore pattern
 */
export class ThreadCommentsStore {
  private commentsBlocks: Y.Map<any> | null = null;
  private commentsDoc: Y.Doc | null = null;
  private observers: Set<() => void> = new Set();

  /**
   * Set the YJS map and doc for thread comments
   */
  setCommentsMap(commentsBlocks: Y.Map<any>, commentsDoc: Y.Doc): void {
    // Clean up old observer if any
    if (this.commentsBlocks) {
      this.commentsBlocks.unobserveDeep(this.handleCommentsChange);
    }

    this.commentsBlocks = commentsBlocks;
    this.commentsDoc = commentsDoc;

    // Set up observer
    this.commentsBlocks.observeDeep(this.handleCommentsChange);

    console.log("💬 [ThreadCommentsStore] Yjs map set and observer attached");
  }

  /**
   * Handle changes to comments
   */
  private handleCommentsChange = (): void => {
    console.log("💬 [ThreadCommentsStore] Yjs change detected, notifying observers");
    this.notifyObservers();
  };

  /**
   * Get all comments from all threads
   */
  getAllComments(): Comment[] {
    if (!this.commentsBlocks) {
      return [];
    }

    const comments: Comment[] = [];

    this.commentsBlocks.forEach((value, key) => {
      // Keys are ${threadId}_comments
      if (key.endsWith("_comments")) {
        // Handle both old format (object with items array) and new format (Y.Array)
        if (value instanceof Y.Array) {
          value.toArray().forEach((comment: Comment) => {
            comments.push(comment);
          });
        } else if (value?.items && Array.isArray(value.items)) {
          // Legacy format support
          value.items.forEach((comment: Comment) => {
            comments.push(comment);
          });
        }
      }
    });

    return comments;
  }

  /**
   * Get comments for a specific thread
   */
  getThreadComments(threadId: string): Comment[] {
    if (!this.commentsBlocks) {
      return [];
    }

    const key = `${threadId}_comments`;
    const commentsData = this.commentsBlocks.get(key);

    // Handle both new format (Y.Array) and legacy format (object with items)
    if (commentsData instanceof Y.Array) {
      return commentsData.toArray();
    } else if (commentsData?.items && Array.isArray(commentsData.items)) {
      // Legacy format - migrate to Y.Array
      console.log('💬 [ThreadCommentsStore] Migrating legacy format to Y.Array');
      const yArray = new Y.Array<Comment>();
      this.commentsDoc!.transact(() => {
        yArray.push(commentsData.items);
        this.commentsBlocks!.set(key, yArray);
      });
      return yArray.toArray();
    }

    return [];
  }

  /**
   * Add a comment to a thread
   * FIXED: Now uses Y.Array.push() instead of replacing entire array
   */
  addComment(threadId: string, comment: Omit<Comment, 'id' | 'timestamp'>): void {
    if (!this.commentsDoc || !this.commentsBlocks) {
      throw new Error("ThreadCommentsStore not properly initialized");
    }

    const newComment: Comment = {
      ...comment,
      id: crypto.randomUUID(),
      timestamp: Date.now()
    };

    this.commentsDoc.transact(() => {
      const key = `${threadId}_comments`;
      let commentsArray = this.commentsBlocks!.get(key);

      // Handle legacy format or create new Y.Array
      if (!commentsArray) {
        // No comments yet - create new Y.Array
        commentsArray = new Y.Array<Comment>();
        this.commentsBlocks!.set(key, commentsArray);
      } else if (!(commentsArray instanceof Y.Array)) {
        // Legacy format - migrate to Y.Array
        console.log('💬 [ThreadCommentsStore] Migrating legacy format during addComment');
        const legacyItems = commentsArray?.items || [];
        commentsArray = new Y.Array<Comment>();
        if (legacyItems.length > 0) {
          commentsArray.push(legacyItems);
        }
        this.commentsBlocks!.set(key, commentsArray);
      }

      // Use Y.Array's push operation for proper CRDT merging
      commentsArray.push([newComment]);
      console.log(`💬 [ThreadCommentsStore] Added comment to thread ${threadId} using Y.Array.push()`);
    });
  }

  /**
   * Subscribe to comment changes
   */
  subscribe(callback: () => void): () => void {
    this.observers.add(callback);
    return () => {
      this.observers.delete(callback);
    };
  }

  /**
   * Notify all observers
   */
  private notifyObservers(): void {
    this.observers.forEach((callback) => callback());
  }

  /**
   * Clean up
   */
  destroy(): void {
    if (this.commentsBlocks) {
      this.commentsBlocks.unobserveDeep(this.handleCommentsChange);
    }

    this.observers.clear();
    this.commentsBlocks = null;
  }
}
