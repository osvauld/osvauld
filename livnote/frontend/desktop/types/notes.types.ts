import { EditorState } from "prosemirror-state";
import { Schema } from "prosemirror-model";
import { Awareness } from "y-protocols/awareness";
import * as Y from "yjs";

// ============================================================================
// COMMENT TYPES 
// ============================================================================

export interface ThreadInfo {
  position: { from: number; to: number };
  createdAt: number;
  resolved: boolean;
}

export interface Reply {
  id: string;
  author: string;  // userId
  authorName: string;
  createdAt: number;
  readBy: string[];  // array of userIds
}

export interface CommentThread {
  id: string;
  threadInfo: ThreadInfo;
  replies: Reply[];
}

/**
 * Position information for comments in the document
 */
export interface CommentPosition {
  from: number;
  to: number;
  node_path?: number[]; // Path to the node for resilient positioning
}

// ============================================================================
// NOTE TYPES
// ============================================================================

export interface NoteContent {
  main_doc: number[];
  image_state: number[];
  comment_state: number[];
  client_id: string;
  last_modified: number;
  title: string;
}

export interface Note {
  id: string;
  data: NoteContent;
  favourite?: boolean;
  folderId?: string;
}

export interface NotePreview {
  id: string;
  title?: string;
  preview: any;
  favourite?: boolean;
  folderId?: string;
  lastModified?: number;
  lastAccessed?: number;
}

// ============================================================================
// IMAGE TYPES
// ============================================================================

export interface ImageMetadata {
  id: string;
  mimeType: string;
  size: number;
  width?: number;
  height?: number;
  uploadedBy: number;
  timestamp: number;
  filename?: string;
}

export interface ImageAsset {
  id: string;
  data: string; // Base64 data
  mimeType: string;
  size: number;
  width?: number;
  height?: number;
  uploadedBy: number;
  timestamp: number;
  filename?: string;
}

// ============================================================================
// USER & COLLABORATION TYPES
// ============================================================================

/**
 * User information for collaboration awareness
 */
export interface UserInfo {
  name: string;
  color: string;
  id: number;
  userId: string;
}

export interface Collaborator {
  id: string;
  name: string;
  color: string;
  clientId: number;
}

// ============================================================================
// EDITOR TYPES
// ============================================================================

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
