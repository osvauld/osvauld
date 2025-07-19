import * as Y from "yjs";
import type {
  Comment,
  CommentThread,
  CommentPosition,
  UpdateCommentParams,
  UserInfo,
  CommentUpdateCallback
} from "../../types/notes.types";

export class CommentsService {
  private commentsMap: Y.Map<CommentThread>;
  private callbacks: Map<string, CommentUpdateCallback[]> = new Map();
  private currentUser: UserInfo | null = null;

  constructor(commentsMap: Y.Map<CommentThread>) {
    this.commentsMap = commentsMap;

    // Set up observer for real-time updates
    this.commentsMap.observe((event) => {
      console.log("observer here", event);
      this.handleCommentsUpdate(event);
    });
  }

  /**
   * Set the current user for comment attribution
   */
  setCurrentUser(user: UserInfo): void {
    this.currentUser = user;
  }

  /**
   * Create a new comment thread
   */
  createThread(position: CommentPosition, content: string): string {
    if (!this.currentUser) {
      console.error('CommentsService: Current user not set!');
      throw new Error("Current user must be set before creating comments");
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

    // Store in Yjs map for real-time sync
    this.commentsMap.set(threadId, thread);

    return threadId;
  }

  /**
   * Add a reply to an existing thread
   */
  addComment(threadId: string, content: string): string | null {
    if (!this.currentUser) {
      throw new Error("Current user must be set before creating comments");
    }

    const thread = this.commentsMap.get(threadId);
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

    // Create updated thread with new comment
    const updatedThread: CommentThread = {
      ...thread,
      comments: [...thread.comments, newComment],
      updated_at: timestamp
    };

    this.commentsMap.set(threadId, updatedThread);

    return commentId;
  }

  /**
   * Update an existing comment
   */
  updateComment(threadId: string, commentId: string, updates: UpdateCommentParams): boolean {
    const thread = this.commentsMap.get(threadId);
    if (!thread) {
      console.error("Thread not found:", threadId);
      return false;
    }

    const commentIndex = thread.comments.findIndex(c => c.id === commentId);
    if (commentIndex === -1) {
      console.error("Comment not found:", commentId);
      return false;
    }

    // Update the comment
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

    this.commentsMap.set(threadId, updatedThread);

    return true;
  }

  /**
   * Resolve or unresolve a thread
   */
  resolveThread(threadId: string, resolved: boolean): boolean {
    const thread = this.commentsMap.get(threadId);
    if (!thread) {
      console.error("Thread not found:", threadId);
      return false;
    }

    const updatedThread: CommentThread = {
      ...thread,
      resolved,
      updated_at: Date.now()
    };

    this.commentsMap.set(threadId, updatedThread);

    return true;
  }

  /**
   * Delete a comment thread
   */
  deleteThread(threadId: string): boolean {
    const exists = this.commentsMap.has(threadId);
    if (exists) {
      this.commentsMap.delete(threadId);
    }
    return exists;
  }

  /**
   * Get a specific thread
   */
  getThread(threadId: string): CommentThread | null {
    return this.commentsMap.get(threadId) || null;
  }

  /**
   * Get all threads
   */
  getAllThreads(): CommentThread[] {
    const threads: CommentThread[] = [];
    this.commentsMap.forEach((thread) => {
      threads.push(thread);
    });
    return threads;
  }

  /**
   * Get threads by position range
   */
  getThreadsInRange(from: number, to: number): CommentThread[] {
    return this.getAllThreads().filter(thread => {
      const pos = thread.position;
      return pos.from >= from && pos.to <= to;
    });
  }

  /**
   * Update thread position (for document changes)
   */
  updateThreadPosition(threadId: string, newPosition: CommentPosition): boolean {
    const thread = this.commentsMap.get(threadId);
    if (!thread) {
      return false;
    }

    const updatedThread: CommentThread = {
      ...thread,
      position: newPosition,
      updated_at: Date.now()
    };

    this.commentsMap.set(threadId, updatedThread);
    return true;
  }

  /**
   * Subscribe to comment updates
   */
  onUpdate(eventType: string, callback: CommentUpdateCallback): void {
    if (!this.callbacks.has(eventType)) {
      this.callbacks.set(eventType, []);
    }
    this.callbacks.get(eventType)!.push(callback);
  }

  /**
   * Unsubscribe from comment updates
   */
  offUpdate(eventType: string, callback: CommentUpdateCallback): void {
    const callbacks = this.callbacks.get(eventType);
    if (callbacks) {
      const index = callbacks.indexOf(callback);
      if (index > -1) {
        callbacks.splice(index, 1);
      }
    }
  }

  /**
   * Handle Yjs updates
   */
  private handleCommentsUpdate(event: Y.YMapEvent<CommentThread>): void {
    event.changes.keys.forEach((change, key) => {
      if (change.action === 'add') {
        this.emitEvent('thread_added', { threadId: key });
      } else if (change.action === 'update') {
        this.emitEvent('thread_updated', { threadId: key });
      } else if (change.action === 'delete') {
        this.emitEvent('thread_deleted', { threadId: key });
      }
    });
  }

  /**
   * Emit events to subscribers
   */
  private emitEvent(eventType: string, data: any): void {
    const callbacks = this.callbacks.get(eventType);
    if (callbacks) {
      callbacks.forEach((callback) => {
        callback(data);
      });
    }
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

  /**
   * Get comment statistics
   */
  getStats(): {
    totalThreads: number;
    totalComments: number;
    resolvedThreads: number;
    activeThreads: number;
  } {
    const threads = this.getAllThreads();
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
} 
