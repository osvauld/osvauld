/**
 * Content Store - Manages shared persistent content
 *
 * This store wraps the contentDoc and provides reactive methods
 * for Publisher mode to create/update/delete content.
 * Viewer mode can only read from this store.
 */

import { loroCoordinator } from './loroCoordinator';
import type { LoroMap } from 'loro-crdt';

class ContentStore {
  /**
   * Get the current contentMap (always fresh)
   */
  private getContentMap(): LoroMap {
    return loroCoordinator.getContentMap();
  }

  /**
   * Get all content as a plain object
   */
  getContent(): Record<string, any> {
    const content: Record<string, any> = {};
    const contentMap = this.getContentMap();

    for (const [key, value] of contentMap.entries()) {
      content[key] = this.deepClone(value);
    }

    return content;
  }

  /**
   * Get a specific content item
   */
  getItem(key: string): any {
    const contentMap = this.getContentMap();
    return contentMap.get(key);
  }

  /**
   * Publisher Action: Add a new post
   */
  publishPost(content: string, author: string = 'Publisher'): string {
    const contentMap = this.getContentMap();
    const posts = contentMap.get('posts') as any[] || [];

    const newPost = {
      id: `post-${Date.now()}`,
      content,
      author,
      timestamp: Date.now(),
      likesCount: 0
    };

    // Add to posts array
    const updatedPosts = [...posts, newPost];
    contentMap.set('posts', updatedPosts);

    console.log('✅ [ContentStore] Published new post:', newPost.id);
    return newPost.id;
  }

  /**
   * Publisher Action: Update an existing post
   */
  updatePost(postId: string, updates: Partial<{ content: string; author: string }>): boolean {
    const contentMap = this.getContentMap();
    const posts = contentMap.get('posts') as any[] || [];

    const postIndex = posts.findIndex(p => p.id === postId);
    if (postIndex === -1) {
      console.warn('❌ [ContentStore] Post not found:', postId);
      return false;
    }

    const updatedPosts = [...posts];
    updatedPosts[postIndex] = {
      ...updatedPosts[postIndex],
      ...updates,
      updatedAt: Date.now()
    };

    contentMap.set('posts', updatedPosts);
    console.log('✅ [ContentStore] Updated post:', postId);
    return true;
  }

  /**
   * Publisher Action: Delete a post
   */
  deletePost(postId: string): boolean {
    const contentMap = this.getContentMap();
    const posts = contentMap.get('posts') as any[] || [];

    const filteredPosts = posts.filter(p => p.id !== postId);
    if (filteredPosts.length === posts.length) {
      console.warn('❌ [ContentStore] Post not found:', postId);
      return false;
    }

    contentMap.set('posts', filteredPosts);
    console.log('✅ [ContentStore] Deleted post:', postId);
    return true;
  }

  /**
   * Publisher Action: Update any content key
   */
  setContent(key: string, value: any): void {
    const contentMap = this.getContentMap();
    contentMap.set(key, value);
    console.log('✅ [ContentStore] Updated content:', key);
  }

  /**
   * Viewer Action: Increment likes (can be done by viewers)
   */
  incrementLikes(postId: string): boolean {
    const contentMap = this.getContentMap();
    const posts = contentMap.get('posts') as any[] || [];

    const postIndex = posts.findIndex(p => p.id === postId);
    if (postIndex === -1) {
      console.warn('❌ [ContentStore] Post not found:', postId);
      return false;
    }

    const updatedPosts = [...posts];
    updatedPosts[postIndex] = {
      ...updatedPosts[postIndex],
      likesCount: (updatedPosts[postIndex].likesCount || 0) + 1
    };

    contentMap.set('posts', updatedPosts);
    console.log('✅ [ContentStore] Incremented likes for post:', postId);
    return true;
  }

  /**
   * Get posts sorted by timestamp (newest first)
   */
  getPosts(): any[] {
    const contentMap = this.getContentMap();
    const posts = contentMap.get('posts') as any[] || [];
    return posts.sort((a, b) => (b.timestamp || 0) - (a.timestamp || 0));
  }

  /**
   * Get a single post by ID
   */
  getPost(postId: string): any | null {
    const contentMap = this.getContentMap();
    const posts = contentMap.get('posts') as any[] || [];
    return posts.find(p => p.id === postId) || null;
  }

  /**
   * Add a comment (can be top-level or a reply)
   */
  addComment(postId: string, content: string, parentCommentId: string | null = null, author: string = 'User'): string {
    const contentMap = this.getContentMap();
    const comments = contentMap.get('comments') as any[] || [];

    const newComment = {
      id: `comment-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      postId,
      parentCommentId,
      content,
      author,
      timestamp: Date.now()
    };

    const updatedComments = [...comments, newComment];
    contentMap.set('comments', updatedComments);

    console.log('✅ [ContentStore] Added comment:', newComment.id);
    return newComment.id;
  }

  /**
   * Delete a comment (and all its replies)
   */
  deleteComment(commentId: string): boolean {
    const contentMap = this.getContentMap();
    const comments = contentMap.get('comments') as any[] || [];

    // Find all comments to delete (this comment + all descendants)
    const toDelete = new Set<string>([commentId]);
    let foundMore = true;

    while (foundMore) {
      foundMore = false;
      for (const comment of comments) {
        if (comment.parentCommentId && toDelete.has(comment.parentCommentId) && !toDelete.has(comment.id)) {
          toDelete.add(comment.id);
          foundMore = true;
        }
      }
    }

    const filteredComments = comments.filter(c => !toDelete.has(c.id));

    if (filteredComments.length === comments.length) {
      console.warn('❌ [ContentStore] Comment not found:', commentId);
      return false;
    }

    contentMap.set('comments', filteredComments);
    console.log('✅ [ContentStore] Deleted comment and replies:', commentId, toDelete.size, 'total');
    return true;
  }

  /**
   * Get all comments for a post, organized into a tree structure
   */
  getCommentsTree(postId: string): any[] {
    const contentMap = this.getContentMap();
    const comments = contentMap.get('comments') as any[] || [];

    // Filter comments for this post
    const postComments = comments.filter(c => c.postId === postId);

    // Build tree structure
    const commentMap = new Map<string, any>();
    const rootComments: any[] = [];

    // First pass: create map of all comments with replies array
    for (const comment of postComments) {
      commentMap.set(comment.id, { ...comment, replies: [] });
    }

    // Second pass: build tree by adding children to parents
    for (const comment of postComments) {
      const commentWithReplies = commentMap.get(comment.id);

      if (comment.parentCommentId) {
        const parent = commentMap.get(comment.parentCommentId);
        if (parent) {
          parent.replies.push(commentWithReplies);
        } else {
          // Parent not found, treat as root
          rootComments.push(commentWithReplies);
        }
      } else {
        // Top-level comment
        rootComments.push(commentWithReplies);
      }
    }

    // Sort root comments by timestamp (newest first)
    rootComments.sort((a, b) => (b.timestamp || 0) - (a.timestamp || 0));

    // Recursively sort replies
    const sortReplies = (comment: any) => {
      if (comment.replies && comment.replies.length > 0) {
        comment.replies.sort((a: any, b: any) => (a.timestamp || 0) - (b.timestamp || 0)); // Oldest first for replies
        comment.replies.forEach(sortReplies);
      }
    };
    rootComments.forEach(sortReplies);

    return rootComments;
  }

  /**
   * Get flat list of comments for a post
   */
  getComments(postId: string): any[] {
    const contentMap = this.getContentMap();
    const comments = contentMap.get('comments') as any[] || [];
    return comments.filter(c => c.postId === postId);
  }

  /**
   * Clear all content (dangerous - publisher only)
   */
  clearContent(): void {
    const contentMap = this.getContentMap();
    contentMap.clear();
    console.log('⚠️ [ContentStore] Cleared all content');
  }

  /**
   * Deep clone utility
   */
  private deepClone(obj: any): any {
    if (obj === null || typeof obj !== 'object') return obj;
    if (obj instanceof Date) return new Date(obj);
    if (Array.isArray(obj)) return obj.map(item => this.deepClone(item));

    const cloned: any = {};
    for (const key in obj) {
      if (obj.hasOwnProperty(key)) {
        cloned[key] = this.deepClone(obj[key]);
      }
    }
    return cloned;
  }

  /**
   * Subscribe to content changes
   */
  subscribe(callback: () => void): () => void {
    const contentMap = this.getContentMap();
    const unsubscribe = contentMap.subscribe(callback);
    return unsubscribe;
  }
}

// Export singleton instance
export const contentStore = new ContentStore();