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
      if (key.endsWith("_comments") && value?.items && Array.isArray(value.items)) {
        value.items.forEach((comment: Comment) => {
          comments.push(comment);
        });
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
    const threadData = this.commentsBlocks.get(key);
    return threadData?.items || [];
  }

  /**
   * Add a comment to a thread
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
      const current = this.commentsBlocks!.get(key) || { items: [] };

      this.commentsBlocks!.set(key, {
        items: [...current.items, newComment]
      });
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
