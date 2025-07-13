import { EditorState } from "prosemirror-state";
import { Schema } from "prosemirror-model";
import { Awareness } from "y-protocols/awareness";
import * as Y from "yjs";

/**
 * Note content structure for storage, retrieval and synchronization
 */
export interface NoteContent {
  content: string | Record<string, unknown>;
  yjs_state: Uint8Array | number[];
  editor_state: string | Record<string, unknown>;
  client_id: string;
  resource_id: string;
  last_modified?: number;
  title?: string;
}

/**
 * Parameters for note creation operations
 */
export interface CreateNoteParams {
  folderId: string;
}

/**
 * User information for collaboration awareness
 */
export interface UserInfo {
  name: string;
  color: string;
  id: number;
}

/**
 * Editor document state for collaboration
 */
export interface EditorDocumentState {
  ydoc: Y.Doc;
  type: Y.XmlFragment;
  awareness: Awareness;
  clientID: number;
  editorState: EditorState | null;
  schema: Schema;
}



/**
 * Note metadata for list displays
 */
export interface NoteMetadata {
  id: string;
  title: string;
  last_modified: number;
  created_at?: number;
  folder_id?: string;
}

/**
 * Note folder information
 */
export interface NoteFolder {
  id: string;
  name: string;
  parent_id?: string;
  created_at?: number;
}

// === Comment System Types ===

/**
 * Position information for comments in the document
 */
export interface CommentPosition {
  from: number;
  to: number;
  node_path?: number[]; // Path to the node for resilient positioning
}

/**
 * Individual comment within a thread
 */
export interface Comment {
  id: string;
  thread_id: string;
  author: UserInfo;
  content: string;
  timestamp: number;
  edited_at?: number;
  resolved?: boolean;
}

/**
 * Comment thread containing multiple comments
 */
export interface CommentThread {
  id: string;
  comments: Comment[];
  resolved: boolean;
  position: CommentPosition;
  created_at: number;
  updated_at: number;
}

/**
 * Comment creation parameters
 */
export interface CreateCommentParams {
  thread_id?: string; // If replying to existing thread
  content: string;
  position?: CommentPosition; // Required for new threads
}

/**
 * Comment update parameters
 */
export interface UpdateCommentParams {
  content?: string;
  resolved?: boolean;
}


/**
 * Comment update callback function signature
 */
export type CommentUpdateCallback = (eventData?: any) => void;

/**
 * Comment event handler map for type-safe event handling
 */
export type CommentEventHandlers = {
  thread_added: CommentUpdateCallback;
  thread_updated: CommentUpdateCallback;
  thread_deleted: CommentUpdateCallback;
  comment_added: CommentUpdateCallback;
  comment_updated: CommentUpdateCallback;
  comment_deleted: CommentUpdateCallback;
}; 
