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
 * Response structure from the server for note operations
 */
export interface NoteResponse {
	data?: {
		id?: string;
		content?: Record<string, unknown>;
		yjs_state?: number[] | Uint8Array;
		editor_state?: Record<string, unknown> | string;
		client_id?: string;
		resource_id?: string;
		last_modified?: number;
		title?: string;
		[key: string]: unknown;
	};
	error?: string;
	status?: 'success' | 'error';
	message?: string;
}

/**
 * Collaboration update event data
 */
export interface CollaborationUpdateEvent {
	update: number[];
	clientID: number;
	client_id: string;
	resource_id: string;
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