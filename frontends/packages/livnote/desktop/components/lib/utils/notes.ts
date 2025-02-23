import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
import { emit } from "@tauri-apps/api/event";
import { baseKeymap } from "prosemirror-commands";
import { history } from "prosemirror-history";
import { keymap } from "prosemirror-keymap";
import { Schema } from "prosemirror-model";
import { schema } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";
import { EditorState } from "prosemirror-state";
import {
  redo,
  undo,
  yCursorPlugin,
  ySyncPlugin,
  yUndoPlugin,
} from "y-prosemirror";
import { Awareness } from 'y-protocols/awareness';
import * as Y from 'yjs';

interface NoteContent {
  content: any;
  yjs_state: Uint8Array;
  editor_state: any;
  client_id: string;
  resource_id: string;
}

interface CreateNoteParams {
  folderId: string;
  clientId: string;
  resourceId: string;
}

export class Notes {
  private ydoc: Y.Doc;
  private type: Y.XmlFragment;
  private awareness: Awareness;
  private clientID: number;
  private currentNoteId: string | null = null;
  private currentClientId: string | null = null;
  private currentResourceId: string | null = null;
  private editorState: EditorState | null = null;
  private editorSchema: Schema;

  constructor() {
    this.clientID = Math.floor(Math.random() * 0xffffffff);
    this.initSchema();
    this.initYjs();
  }

  private initSchema() {
    const nodes = addListNodes(schema.spec.nodes, "paragraph block*", "block");
    const marks = {
      ...schema.spec.marks,
      textColor: {
        attrs: { color: { default: "" } },
        parseDOM: [
          {
            style: "color",
            getAttrs: (value) => ({ color: value }),
          },
        ],
        toDOM: (mark) => ["span", { style: `color: ${mark.attrs.color}` }, 0],
      },
    };
    this.editorSchema = new Schema({ nodes, marks });
  }

  private initYjs() {
    this.ydoc = new Y.Doc();
    this.type = this.ydoc.getXmlFragment('prosemirror');
    this.awareness = new Awareness(this.ydoc);

    this.awareness.setLocalState({
      user: {
        name: `User ${this.clientID}`,
        color: `#${Math.floor(Math.random() * 16777215).toString(16)}`,
      },
    });
  }

  private initEditorState() {
    this.editorState = EditorState.create({
      schema: this.editorSchema,
      plugins: [
        history(),
        keymap(baseKeymap),
        ySyncPlugin(this.type),
        yCursorPlugin(this.awareness),
        yUndoPlugin(),
        keymap({
          "Mod-z": undo,
          "Mod-y": redo,
          "Mod-Shift-z": redo,
        }),
      ],
    });
  }

  // Step 1: Create empty credential
  async createEmptyCredential({ folderId, clientId, resourceId }: CreateNoteParams): Promise<string> {
    try {
      // Just create an empty credential first
      const response = await sendMessage("addCredential", {
        credentialPayload: JSON.stringify({}),
        folderId: folderId,
        credentialType: "notes"
      });

      // Store the IDs
      this.currentNoteId = response;
      this.currentClientId = clientId;
      this.currentResourceId = resourceId;

      return response.id;
    } catch (error) {
      console.error("Error creating empty credential:", error);
      throw error;
    }
  }

  // Step 2: Initialize editor state and update credential
  async initializeNoteState() {
    if (!this.currentNoteId) {
      throw new Error("No note ID available");
    }

    try {
      // Initialize fresh Yjs and editor state
      this.initYjs();
      this.initEditorState();

      // Create initial content with all states
      const noteContent: NoteContent = {
        content: {},
        yjs_state: Array.from(Y.encodeStateAsUpdate(this.ydoc)),
        editor_state: this.editorState?.toJSON() || null,
        client_id: this.currentClientId || '',
        resource_id: this.currentResourceId || ''
      };

      console.log(this.currentNoteId);
      // Update the credential with initialized states
      const response = await sendMessage("updateCredential", {
        id: this.currentNoteId,
        data: JSON.stringify(noteContent),
      });

      console.log(response)


      return this.getDoc();
    } catch (error) {
      console.error("Error initializing note state:", error);
      throw error;
    }
  }
  getDoc() {
    return {
      ydoc: this.ydoc,
      type: this.type,
      awareness: this.awareness,
      clientID: this.clientID,
      editorState: this.editorState,
      schema: this.editorSchema
    };
  }

  updateEditorState(newState: EditorState) {
    this.editorState = newState;
  }

  async saveNote() {
    if (!this.currentNoteId || !this.editorState) {
      console.error("No note is currently active or editor state is missing");
      return;
    }

    try {
      const yjs_state = Y.encodeStateAsUpdate(this.ydoc);
      const editorJSON = this.editorState.toJSON();
      const content = this.type.toJSON();

      const noteContent: NoteContent = {
        content,
        yjs_state: Array.from(yjs_state),
        editor_state: editorJSON,
        client_id: this.currentClientId || '',
        resource_id: this.currentResourceId || ''
      };

      await sendMessage("updateCredential", {
        id: this.currentNoteId,
        data: JSON.stringify(noteContent),
      });

    } catch (error) {
      console.error("Error saving note:", error);
      throw error;
    }
  }

  async loadNote(noteId: string) {
    try {
      const response = await sendMessage("getCredential", {
        credentialId: noteId
      });

      if (!response || !response.data) {
        throw new Error("Note not found");
      }

      this.currentNoteId = noteId;

      const noteContent: NoteContent = JSON.parse(response.data.credentialPayload);

      this.currentClientId = noteContent.client_id;
      this.currentResourceId = noteContent.resource_id;

      // Clear and reinitialize Yjs document
      this.ydoc.destroy();
      this.initYjs();

      // Apply saved Yjs state
      if (noteContent.yjs_state) {
        const yjs_state = new Uint8Array(noteContent.yjs_state);
        Y.applyUpdate(this.ydoc, yjs_state);
      }

      // Restore editor state if available
      if (noteContent.editor_state) {
        this.editorState = EditorState.fromJSON({
          schema: this.editorSchema,
          plugins: this.editorState?.plugins || []
        }, noteContent.editor_state);
      }

      // If there's content but no states, initialize from content
      if (noteContent.content && !noteContent.yjs_state && !noteContent.editor_state) {
        this.type.applyDelta(noteContent.content);
      }

      return this.getDoc();

    } catch (error) {
      console.error("Error loading note:", error);
      throw error;
    }
  }

  async createNote({ folderId, clientId, resourceId }: CreateNoteParams): Promise<string> {
    try {
      this.currentClientId = clientId;
      this.currentResourceId = resourceId;

      const initialContent: NoteContent = {
        content: {},
        yjs_state: Array.from(Y.encodeStateAsUpdate(this.ydoc)),
        editor_state: this.editorState?.toJSON() || null,
        client_id: clientId,
        resource_id: resourceId
      };

      const response = await sendMessage("addCredential", {
        credentialPayload: JSON.stringify(initialContent),
        folderId: folderId,
        credentialType: "notes"
      });

      this.currentNoteId = response.id;
      return response.id;

    } catch (error) {
      console.error("Error creating note:", error);
      throw error;
    }
  }

  async handleCollaborationUpdate(update: any) {
    try {
      await emit("sync-update", JSON.stringify({
        update: Array.from(update),
        clientID: this.clientID,
        client_id: this.currentClientId,
        resource_id: this.currentResourceId
      }));

      await this.saveNote();
    } catch (error) {
      console.error("Error handling collaboration update:", error);
      throw error;
    }
  }

  applyUpdate(update: Uint8Array | number[], sender: number) {
    if (sender === this.clientID) return;

    try {
      const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);
      Y.applyUpdate(this.ydoc, updateArray);
    } catch (error) {
      console.error("Error applying update:", error);
      throw error;
    }
  }

  destroy() {
    if (this.ydoc) {
      this.ydoc.destroy();
    }
  }
}

export const notesInstance = new Notes();
