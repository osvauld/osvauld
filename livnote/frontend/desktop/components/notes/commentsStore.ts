import * as Y from "yjs";
import type {
  Comment,
  CommentThread,
  CommentPosition,
  UpdateCommentParams,
  UserInfo
} from "../../types/notes.types";

export class CommentsStore {
  private currentCommentsMap: Y.Map<CommentThread> | null = null;
  private subscribers: Set<() => void> = new Set();
  private currentUser: UserInfo | null = null;
  private mapObserver: ((event: any) => void) | null = null;

  /**
   * Set the comments map for the current note
   */
  setCommentsMap(map: Y.Map<CommentThread>): void {
    if (this.currentCommentsMap && this.mapObserver) {
      this.currentCommentsMap.unobserve(this.mapObserver);
    }

    this.currentCommentsMap = map;

    this.mapObserver = () => {
      this.notifySubscribers();
    };
    map.observe(this.mapObserver);
    this.notifySubscribers();
  }

  /**
   * Get current comments as array
   */
  getComments(): CommentThread[] {
    if (!this.currentCommentsMap) return [];

    const threads: CommentThread[] = [];
    this.currentCommentsMap.forEach((thread, key) => {
      threads.push(thread);
    });

    return threads;
  }

  /**
   * Subscribe to comment updates
   */
  subscribe(callback: () => void): () => void {
    this.subscribers.add(callback);
    return () => this.subscribers.delete(callback);
  }

  /**
   * Set current user for operations
   */
  setCurrentUser(user: UserInfo): void {
    this.currentUser = user;
  }

  /**
   * Create a new comment thread
   */
  createThread(position: CommentPosition, content: string): string {
    if (!this.currentUser) {
      throw new Error("Current user must be set before creating comments");
    }

    if (!this.currentCommentsMap) {
      throw new Error("Comments map not initialized");
    }

    const threadId = this.generateThreadId();
    const commentId = this.generateCommentId();
    const timestamp = Date.now();

    const comment: Comment = {
      id: commentId,
      thread_id: threadId,
      author: this.currentUser,
      content,
      timestamp,
      resolved: false
    };

    const thread: CommentThread = {
      id: threadId,
      comments: [comment],
      resolved: false,
      position,
      created_at: timestamp,
      updated_at: timestamp
    };

    this.currentCommentsMap.set(threadId, thread);
    return threadId;
  }

  /**
   * Add a reply to an existing thread
   */
  addComment(threadId: string, content: string): string | null {
    if (!this.currentUser) {
      throw new Error("Current user must be set before creating comments");
    }

    if (!this.currentCommentsMap) {
      throw new Error("Comments map not initialized");
    }

    const thread = this.currentCommentsMap.get(threadId);
    if (!thread) {
      console.error("Thread not found:", threadId);
      return null;
    }

    const commentId = this.generateCommentId();
    const timestamp = Date.now();

    const newComment: Comment = {
      id: commentId,
      thread_id: threadId,
      author: this.currentUser,
      content,
      timestamp,
      resolved: false
    };

    const updatedThread: CommentThread = {
      ...thread,
      comments: [...thread.comments, newComment],
      updated_at: timestamp
    };

    this.currentCommentsMap.set(threadId, updatedThread);
    return commentId;
  }

  /**
   * Update an existing comment
   */
  updateComment(threadId: string, commentId: string, updates: UpdateCommentParams): boolean {
    if (!this.currentCommentsMap) return false;

    const thread = this.currentCommentsMap.get(threadId);
    if (!thread) {
      console.error("Thread not found:", threadId);
      return false;
    }

    const commentIndex = thread.comments.findIndex(c => c.id === commentId);
    if (commentIndex === -1) {
      console.error("Comment not found:", commentId);
      return false;
    }

    const updatedComments = [...thread.comments];
    updatedComments[commentIndex] = {
      ...updatedComments[commentIndex],
      ...updates,
      edited_at: Date.now()
    };

    const updatedThread: CommentThread = {
      ...thread,
      comments: updatedComments,
      updated_at: Date.now()
    };

    this.currentCommentsMap.set(threadId, updatedThread);
    return true;
  }

  /**
   * Resolve or unresolve a thread
   */
  resolveThread(threadId: string, resolved: boolean): boolean {
    if (!this.currentCommentsMap) return false;

    const thread = this.currentCommentsMap.get(threadId);
    if (!thread) {
      console.error("Thread not found:", threadId);
      return false;
    }

    const updatedThread: CommentThread = {
      ...thread,
      resolved,
      updated_at: Date.now()
    };

    this.currentCommentsMap.set(threadId, updatedThread);
    return true;
  }

  /**
   * Delete a comment thread
   */
  deleteThread(threadId: string): boolean {
    if (!this.currentCommentsMap) return false;

    const exists = this.currentCommentsMap.has(threadId);
    if (exists) {
      this.currentCommentsMap.delete(threadId);
    }
    return exists;
  }

  /**
   * Get a specific thread
   */
  getThread(threadId: string): CommentThread | null {
    if (!this.currentCommentsMap) return null;
    return this.currentCommentsMap.get(threadId) || null;
  }

  /**
   * Get threads by position range
   */
  getThreadsInRange(from: number, to: number): CommentThread[] {
    return this.getComments().filter(thread => {
      const pos = thread.position;
      return pos.from >= from && pos.to <= to;
    });
  }

  /**
   * Update thread position (for document changes)
   */
  updateThreadPosition(threadId: string, newPosition: CommentPosition): boolean {
    if (!this.currentCommentsMap) return false;

    const thread = this.currentCommentsMap.get(threadId);
    if (!thread) {
      return false;
    }

    const updatedThread: CommentThread = {
      ...thread,
      position: newPosition,
      updated_at: Date.now()
    };

    this.currentCommentsMap.set(threadId, updatedThread);
    return true;
  }

  /**
   * Get comment statistics
   */
  getStats(): {
    totalThreads: number;
    totalComments: number;
    resolvedThreads: number;
    activeThreads: number;
  } {
    const threads = this.getComments();
    const totalThreads = threads.length;
    const resolvedThreads = threads.filter(t => t.resolved).length;
    const totalComments = threads.reduce((sum, t) => sum + t.comments.length, 0);

    return {
      totalThreads,
      totalComments,
      resolvedThreads,
      activeThreads: totalThreads - resolvedThreads
    };
  }

  /**
   * Check if store is initialized
   */
  isInitialized(): boolean {
    return this.currentCommentsMap !== null;
  }

  /**
   * Clean up resources
   */
  destroy(): void {
    if (this.currentCommentsMap && this.mapObserver) {
      this.currentCommentsMap.unobserve(this.mapObserver);
    }
    this.subscribers.clear();
    this.currentCommentsMap = null;
    this.mapObserver = null;
  }

  /**
   * Notify all subscribers of changes
   */
  private notifySubscribers(): void {
    this.subscribers.forEach(callback => callback());
  }

  /**
   * Generate unique thread ID
   */
  private generateThreadId(): string {
    return 'thread_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9);
  }

  /**
   * Generate unique comment ID
   */
  private generateCommentId(): string {
    return 'comment_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9);
  }
}
