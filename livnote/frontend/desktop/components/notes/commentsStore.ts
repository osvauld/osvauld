import * as Y from "yjs";
import type { UserInfo, ThreadInfo, Reply, CommentThread } from "../../types/notes.types";

/**
 * Manages comments using YJS for collaborative editing
 * Uses separate document with nested YJS types for threads and replies
 */
export class CommentsStore {
  private threads: Y.Map<Y.Map<any>> | null = null;
  private replies: Y.Map<Y.Array<Y.Map<any>>> | null = null;
  private replyContents: Y.Map<Y.XmlFragment> | null = null;
  private currentUser: UserInfo | null = null;
  private observers: Set<() => void> = new Set();

  /**
   * Set the YJS maps for comments (threads, replies, and content)
   */
  setCommentsMap(
    threads: Y.Map<Y.Map<any>>,
    replies: Y.Map<Y.Array<Y.Map<any>>>,
    replyContents: Y.Map<Y.XmlFragment>
  ): void {
    // Clean up old observers if any
    if (this.threads) {
      this.threads.unobserveDeep(this.handleCommentsChange);
    }
    if (this.replies) {
      this.replies.unobserveDeep(this.handleCommentsChange);
    }
    if (this.replyContents) {
      this.replyContents.unobserveDeep(this.handleCommentsChange);
    }

    this.threads = threads;
    this.replies = replies;
    this.replyContents = replyContents;

    // Set up observers
    this.threads.observeDeep(this.handleCommentsChange);
    this.replies.observeDeep(this.handleCommentsChange);
    this.replyContents.observeDeep(this.handleCommentsChange);
  }

  /**
   * Handle changes to comments
   */
  private handleCommentsChange = (): void => {
    this.notifyObservers();
  };

  /**
   * Set current user
   */
  setCurrentUser(user: UserInfo): void {
    this.currentUser = user;
  }

  /**
   * Create a new comment thread with initial reply
   */
  createThread(position: { from: number; to: number }, initialContent: string): string {
    if (!this.threads || !this.replies || !this.replyContents || !this.currentUser) {
      throw new Error("CommentsStore not properly initialized");
    }

    const threadId = this.generateThreadId();
    const replyId = this.generateReplyId();
    const now = Date.now();

    // Create thread metadata as Y.Map
    const threadMap = new Y.Map<any>();
    threadMap.set("position", position);
    threadMap.set("createdAt", now);
    threadMap.set("resolved", false);

    // Create first reply as Y.Map
    const replyMap = new Y.Map<any>();
    replyMap.set("id", replyId);
    replyMap.set("author", this.currentUser.userId);
    replyMap.set("createdAt", now);

    const readByArray = new Y.Array<string>();
    readByArray.push([this.currentUser.userId]);
    replyMap.set("readBy", readByArray);

    // Create rich text content for reply
    const contentFragment = new Y.XmlFragment();
    const textElement = new Y.XmlText();
    textElement.insert(0, initialContent);

    const paragraph = new Y.XmlElement('paragraph');
    paragraph.insert(0, [textElement]);
    contentFragment.insert(0, [paragraph]);

    // Store everything
    this.threads.set(threadId, threadMap);

    const repliesArray = new Y.Array<Y.Map<any>>();
    repliesArray.push([replyMap]);
    this.replies.set(threadId, repliesArray);

    this.replyContents.set(replyId, contentFragment);

    return threadId;
  }

  /**
   * Add a reply to an existing thread
   */
  addReply(threadId: string, content: string): string {
    if (!this.threads || !this.replies || !this.replyContents || !this.currentUser) {
      throw new Error("CommentsStore not properly initialized");
    }

    const threadMap = this.threads.get(threadId);
    const repliesArray = this.replies.get(threadId);

    if (!threadMap || !repliesArray) {
      throw new Error(`Thread ${threadId} not found`);
    }

    const replyId = this.generateReplyId();
    const now = Date.now();

    // Create reply as Y.Map
    const replyMap = new Y.Map<any>();
    replyMap.set("id", replyId);
    replyMap.set("author", this.currentUser.userId);
    replyMap.set("createdAt", now);

    const readByArray = new Y.Array<string>();
    readByArray.push([this.currentUser.userId]);
    replyMap.set("readBy", readByArray);

    // Create rich text content
    const contentFragment = new Y.XmlFragment();
    const textElement = new Y.XmlText();
    textElement.insert(0, content);

    const paragraph = new Y.XmlElement('paragraph');
    paragraph.insert(0, [textElement]);
    contentFragment.insert(0, [paragraph]);

    // Add to arrays
    repliesArray.push([replyMap]);
    this.replyContents.set(replyId, contentFragment);

    return replyId;
  }

  /**
   * Mark a thread as read by current user (marks all replies as read)
   */
  markThreadAsRead(threadId: string): void {
    if (!this.replies || !this.currentUser) {
      throw new Error("CommentsStore not properly initialized");
    }

    const repliesArray = this.replies.get(threadId);
    if (!repliesArray) {
      throw new Error(`Thread ${threadId} not found`);
    }

    // Mark each reply as read
    repliesArray.forEach((replyMap) => {
      const readByArray = replyMap.get("readBy") as Y.Array<string>;
      const readBy = readByArray.toArray();

      if (!readBy.includes(this.currentUser!.userId)) {
        readByArray.push([this.currentUser!.userId]);
      }
    });
  }

  /**
   * Mark a specific reply as read by current user
   */
  markReplyAsRead(threadId: string, replyId: string): void {
    if (!this.replies || !this.currentUser) {
      throw new Error("CommentsStore not properly initialized");
    }

    const repliesArray = this.replies.get(threadId);
    if (!repliesArray) {
      throw new Error(`Thread ${threadId} not found`);
    }

    const replyMap = repliesArray.toArray().find(r => r.get("id") === replyId);
    if (!replyMap) {
      throw new Error(`Reply ${replyId} not found in thread ${threadId}`);
    }

    const readByArray = replyMap.get("readBy") as Y.Array<string>;
    const readBy = readByArray.toArray();

    if (!readBy.includes(this.currentUser.userId)) {
      readByArray.push([this.currentUser.userId]);
    }
  }

  /**
   * Get all threads with unread replies for current user
   */
  getUnreadThreads(): CommentThread[] {
    if (!this.threads || !this.replies || !this.currentUser) {
      return [];
    }

    const unread: CommentThread[] = [];

    this.threads.forEach((threadMap, threadId) => {
      const repliesArray = this.replies!.get(threadId);
      if (!repliesArray) return;

      const hasUnread = repliesArray.toArray().some(replyMap => {
        const readByArray = replyMap.get("readBy") as Y.Array<string>;
        const readBy = readByArray.toArray();
        return !readBy.includes(this.currentUser!.userId);
      });

      if (hasUnread) {
        const thread = this.buildCommentThread(threadId, threadMap, repliesArray);
        if (thread) unread.push(thread);
      }
    });

    return unread;
  }

  /**
   * Get a specific comment thread with all its replies
   */
  getThread(threadId: string): CommentThread | null {
    if (!this.threads || !this.replies) {
      return null;
    }

    const threadMap = this.threads.get(threadId);
    const repliesArray = this.replies.get(threadId);

    if (!threadMap || !repliesArray) {
      return null;
    }

    return this.buildCommentThread(threadId, threadMap, repliesArray);
  }

  /**
   * Build a CommentThread object from YJS structures
   */
  private buildCommentThread(
    threadId: string,
    threadMap: Y.Map<any>,
    repliesArray: Y.Array<Y.Map<any>>
  ): CommentThread | null {
    const threadInfo: ThreadInfo = {
      position: threadMap.get("position"),
      createdAt: threadMap.get("createdAt"),
      resolved: threadMap.get("resolved")
    };

    const replies: Reply[] = repliesArray.toArray().map(replyMap => {
      const readByArray = replyMap.get("readBy") as Y.Array<string>;
      return {
        id: replyMap.get("id"),
        author: replyMap.get("author"),
        createdAt: replyMap.get("createdAt"),
        readBy: readByArray.toArray()
      };
    });

    return {
      id: threadId,
      threadInfo,
      replies
    };
  }

  /**
   * Get all comment threads
   */
  getAllThreads(): CommentThread[] {
    if (!this.threads || !this.replies) {
      return [];
    }

    const threads: CommentThread[] = [];

    this.threads.forEach((threadMap, threadId) => {
      const repliesArray = this.replies!.get(threadId);
      if (repliesArray) {
        const thread = this.buildCommentThread(threadId, threadMap, repliesArray);
        if (thread) threads.push(thread);
      }
    });

    return threads;
  }

  /**
   * Resolve a comment thread
   */
  resolveThread(threadId: string): void {
    if (!this.threads) {
      throw new Error("CommentsStore not properly initialized");
    }

    const threadMap = this.threads.get(threadId);
    if (!threadMap) {
      throw new Error(`Thread ${threadId} not found`);
    }

    threadMap.set("resolved", true);
  }

  /**
   * Unresolve a comment thread
   */
  unresolveThread(threadId: string): void {
    if (!this.threads) {
      throw new Error("CommentsStore not properly initialized");
    }

    const threadMap = this.threads.get(threadId);
    if (!threadMap) {
      throw new Error(`Thread ${threadId} not found`);
    }

    threadMap.set("resolved", false);
  }

  /**
   * Delete a comment thread and all its replies
   */
  deleteThread(threadId: string): void {
    if (!this.threads || !this.replies || !this.replyContents) {
      throw new Error("CommentsStore not properly initialized");
    }

    // Get all reply IDs to delete their content
    const repliesArray = this.replies.get(threadId);
    if (repliesArray) {
      repliesArray.toArray().forEach(replyMap => {
        const replyId = replyMap.get("id");
        this.replyContents!.delete(replyId);
      });
    }

    // Delete thread and replies
    this.threads.delete(threadId);
    this.replies.delete(threadId);
  }

  /**
   * Get rich text content fragment for a reply
   */
  getReplyContent(replyId: string): Y.XmlFragment | null {
    if (!this.replyContents) {
      return null;
    }
    return this.replyContents.get(replyId) || null;
  }

  /**
   * Extract plain text content from a reply
   */
  getReplyPlainText(replyId: string): string {
    const contentFragment = this.getReplyContent(replyId);
    if (!contentFragment) {
      return "";
    }

    let text = "";

    const extractText = (node: Y.XmlElement | Y.XmlText): void => {
      if (node instanceof Y.XmlText) {
        text += node.toString();
      } else if (node instanceof Y.XmlElement) {
        node.forEach((child) => {
          extractText(child as Y.XmlElement | Y.XmlText);
        });
      }
    };

    contentFragment.forEach((node) => {
      extractText(node as Y.XmlElement | Y.XmlText);
      text += "\n";
    });

    return text.trim();
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
   * Generate unique thread ID
   */
  private generateThreadId(): string {
    return `thread-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
  }

  /**
   * Generate unique reply ID
   */
  private generateReplyId(): string {
    return `reply-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
  }

  /**
   * Clean up
   */
  destroy(): void {
    if (this.threads) {
      this.threads.unobserveDeep(this.handleCommentsChange);
    }
    if (this.replies) {
      this.replies.unobserveDeep(this.handleCommentsChange);
    }
    if (this.replyContents) {
      this.replyContents.unobserveDeep(this.handleCommentsChange);
    }

    this.observers.clear();
    this.threads = null;
    this.replies = null;
    this.replyContents = null;
    this.currentUser = null;
  }
}
