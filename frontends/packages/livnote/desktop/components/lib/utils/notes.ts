import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
import { emit } from "@tauri-apps/api/event";
import { baseKeymap } from "prosemirror-commands";
import { history } from "prosemirror-history";
import { keymap } from "prosemirror-keymap";
import { Schema } from "prosemirror-model";
import { schema } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";
import { EditorState } from "prosemirror-state";
import { exampleSetup } from "prosemirror-example-setup"
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
    this.editorSchema = new Schema({
      nodes: addListNodes(schema.spec.nodes, "paragraph block*", "block"),
      marks: schema.spec.marks,
    });
  }

  private initYjs() {
    this.ydoc = new Y.Doc();
    this.type = this.ydoc.getXmlFragment('prosemirror');
    this.awareness = new Awareness(this.ydoc);

    // Set up observer for document updates with origin tracking
    this.ydoc.on('update', (update: Uint8Array, origin: any) => {
      // Only handle updates that originated locally (not from sync)
      if (origin !== 'sync') {
        void this.handleCollaborationUpdate(update);
      }
    });

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
        ...exampleSetup({ schema: this.editorSchema }),
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

  async createEmptyCredential({ folderId, clientId, resourceId }: CreateNoteParams): Promise<string> {
    try {
      const response = await sendMessage("addCredential", {
        credentialPayload: JSON.stringify({}),
        folderId: folderId,
        credentialType: "notes"
      });

      this.currentNoteId = response;
      this.currentClientId = clientId;
      this.currentResourceId = resourceId;

      return response;
    } catch (error) {
      console.error("Error creating empty credential:", error);
      throw error;
    }
  }

  async initializeNoteState() {
    if (!this.currentNoteId) {
      throw new Error("No note ID available");
    }

    try {
      this.initYjs();
      this.initEditorState();

      const noteContent: NoteContent = {
        content: {},
        yjs_state: Y.encodeStateAsUpdate(this.ydoc),
        editor_state: this.editorState?.toJSON() || null,
        client_id: this.currentClientId || '',
        resource_id: this.currentResourceId || ''
      };

      await sendMessage("updateCredential", {
        id: this.currentNoteId,
        data: JSON.stringify({
          ...noteContent,
          yjs_state: Array.from(noteContent.yjs_state)
        }),
      });

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
        yjs_state,
        editor_state: editorJSON,
        client_id: this.currentClientId || '',
        resource_id: this.currentResourceId || ''
      };

      await sendMessage("updateCredential", {
        id: this.currentNoteId,
        data: JSON.stringify({
          ...noteContent,
          yjs_state: Array.from(yjs_state)
        }),
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
      const noteContent = response.data; // Using directly as it's already an object

      this.currentClientId = noteContent.client_id;
      this.currentResourceId = noteContent.resource_id;

      // Initialize fresh Yjs document
      this.ydoc.destroy();
      this.initYjs();

      // Initialize fresh editor state with plugins
      this.initEditorState();

      // Get current editor state to ensure plugins are set up
      const currentState = this.editorState;
      if (!currentState) {
        throw new Error("Failed to initialize editor state");
      }

      // Return fresh state for new document
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

      const yjs_state = Y.encodeStateAsUpdate(this.ydoc);

      const initialContent: NoteContent = {
        content: {},
        yjs_state,
        editor_state: this.editorState?.toJSON() || null,
        client_id: clientId,
        resource_id: resourceId
      };

      const response = await sendMessage("addCredential", {
        credentialPayload: JSON.stringify({
          ...initialContent,
          yjs_state: Array.from(yjs_state)
        }),
        folderId: folderId,
        credentialType: "notes"
      });

      this.currentNoteId = response;
      return response;

    } catch (error) {
      console.error("Error creating note:", error);
      throw error;
    }
  }
  async handleCollaborationUpdate(update: Uint8Array) {
    try {
      if (update.length === 0) {
        console.warn("Received empty update");
        return;
      }

      const updateArray = Array.from(update);
      console.log("Sending update:", updateArray);

      await emit("sync-update", {
        update: updateArray,
        clientID: this.clientID,
        client_id: this.currentClientId,
        resource_id: this.currentResourceId
      });

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

      if (updateArray.length === 0) {
        console.warn("Received empty update to apply");
        return;
      }

      console.log("Applying remote update:", Array.from(updateArray));
      // Apply update with 'sync' origin to prevent loop
      Y.applyUpdate(this.ydoc, updateArray, 'sync');
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
