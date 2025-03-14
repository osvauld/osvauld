import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
import { emit } from "@tauri-apps/api/event";
import { baseKeymap, setBlockType } from "prosemirror-commands";
import { keymap } from "prosemirror-keymap";
import { Schema } from "prosemirror-model";
import { schema } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";
import { EditorState } from "prosemirror-state";
import { exampleSetup } from "prosemirror-example-setup";
import { slashCommandPlugin } from "./slashCommandPlugin";

import {
  redo,
  undo,
  yCursorPlugin,
  ySyncPlugin,
  yUndoPlugin,
  yXmlFragmentToProsemirror,
  prosemirrorToYXmlFragment,
  initProseMirrorDoc,
} from "y-prosemirror";
import { Awareness } from "y-protocols/awareness";
import * as Y from "yjs";

interface NoteContent {
  content: any;
  yjs_state: Uint8Array | number[];
  editor_state: any;
  client_id: string;
  resource_id: string;
  last_modified?: number;
}

interface CreateNoteParams {
  folderId: string;
}

export class Notes {
  private ydoc: Y.Doc;
  private type: Y.XmlFragment;
  private awareness: Awareness;
  private clientID: number;
  private currentNoteId: string | null = null;
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
    this.type = this.ydoc.getXmlFragment("prosemirror");
    this.awareness = new Awareness(this.ydoc);

    // Set up observer for document updates with origin tracking
    this.ydoc.on("update", (update: Uint8Array, origin: any) => {
      // Only handle updates that originated locally (not from sync)
      if (origin !== "sync") {
        void this.handleCollaborationUpdate(update);
      }
    });

    // Generate a better color for this user
    const colors = [
      "#FF5630", // Red
      "#FFAB00", // Yellow
      "#36B37E", // Green
      "#00B8D9", // Blue
      "#6554C0", // Purple
      "#FF7452", // Orange
    ];
    const userColor = colors[Math.floor(Math.random() * colors.length)];

    // Set enhanced local user state
    this.awareness.setLocalState({
      user: {
        name: `User ${this.clientID}`,
        color: userColor,
        id: this.clientID,
      },
    });
  }

  public updateUserInfo(name, color) {
    const currentState = this.awareness.getLocalState();
    if (!currentState || !currentState.user) return;

    const newUser = {
      ...currentState.user,
    };

    if (name) {
      newUser.name = name;
    }

    if (color) {
      newUser.color = color;
    }

    this.awareness.setLocalState({
      ...currentState,
      user: newUser,
    });
  }

  private createBasicCustomCursor(user) {
    const cursor = document.createElement("span");
    cursor.classList.add("ProseMirror-yjs-cursor");

    // Set cursor color based on user's color
    cursor.setAttribute("style", `border-color: ${user.color}`);

    // Create the tooltip that shows user name
    const userDiv = document.createElement("div");
    userDiv.setAttribute("style", `background-color: ${user.color}`);

    // Add user name
    userDiv.insertBefore(document.createTextNode(user.name || "Unknown"), null);

    // Attach the tooltip to the cursor
    cursor.insertBefore(userDiv, null);

    return cursor;
  }

  private initEditorState() {
    // Create a synchronized editor state that works with our Yjs document
    try {
      // First create the sync plugin - it's critical this is done before the state is created
      const syncPlugin = ySyncPlugin(this.type);

      // Use the correct function to create a ProseMirror document from YXmlFragment
      let prosemirrorDoc;
      try {
        // Try to initialize from the YJS content
        prosemirrorDoc = yXmlFragmentToProsemirror(
          this.editorSchema,
          this.type,
        );
        console.log("Successfully created ProseMirror doc from YJS content");

        // ADD THIS CODE: Check if the entire document is a single heading node
        if (
          prosemirrorDoc.childCount === 1 &&
          prosemirrorDoc.firstChild &&
          prosemirrorDoc.firstChild.type.name === "heading"
        ) {
          console.log(
            "Fixing document structure - converting from single heading to multiple paragraphs",
          );

          // Get the text content from the heading
          const headingContent = prosemirrorDoc.firstChild.textContent;

          // Create an array of paragraphs from the content
          // Split by double newlines or hard breaks
          const paragraphTexts = headingContent.split(/\n\n|\r\n\r\n/);

          // Create paragraph nodes for each piece of content
          const paragraphNodes = paragraphTexts.map((text) =>
            this.editorSchema.node("paragraph", {}, [
              this.editorSchema.text(text.trim()),
            ]),
          );

          // If no paragraphs were created (empty content), create one empty paragraph
          if (paragraphNodes.length === 0) {
            paragraphNodes.push(this.editorSchema.node("paragraph", {}, []));
          }

          // Create a new document with proper paragraph structure
          prosemirrorDoc = this.editorSchema.node("doc", {}, paragraphNodes);

          console.log(
            "Document structure fixed with " +
            paragraphNodes.length +
            " paragraphs",
          );
        }
      } catch (err) {
        console.error("Error creating ProseMirror doc from YJS:", err);
        // If that fails, create a new empty document
        prosemirrorDoc = initProseMirrorDoc(this.editorSchema);
        console.log("Created empty ProseMirror doc instead");
      }

      // Create the editor state with the document
      this.editorState = EditorState.create({
        schema: this.editorSchema,
        doc: prosemirrorDoc, // Use the document from Yjs
        plugins: [
          slashCommandPlugin(this.editorSchema),
          ...exampleSetup({ schema: this.editorSchema }),
          keymap(baseKeymap),
          syncPlugin, // Use the pre-initialized sync plugin
          yCursorPlugin(this.awareness, {
            cursorBuilder: this.createBasicCustomCursor.bind(this),
          }),
          yUndoPlugin(),
          keymap({
            "Mod-z": undo,
            "Mod-y": redo,
            "Mod-Shift-z": redo,
          }),
        ],
      });

      console.log("Successfully initialized editor state with Yjs content");
    } catch (error) {
      console.error("Error initializing editor state:", error);
      // Create a backup state without Yjs content
      this.editorState = EditorState.create({
        schema: this.editorSchema,
        plugins: [
          ...exampleSetup({ schema: this.editorSchema }),
          keymap(baseKeymap),
          ySyncPlugin(this.type),
          yCursorPlugin(this.awareness, {
            cursorBuilder: this.createBasicCustomCursor.bind(this),
          }),
          yUndoPlugin(),
          keymap({
            "Mod-z": undo,
            "Mod-y": redo,
            "Mod-Shift-z": redo,
          }),
        ],
      });
    }
  }
  /**
   * Creates a new note with initialized state in a single operation
   * @param params Parameters for note creation
   * @returns The ID of the created note
   */
  async createNote({ folderId }: CreateNoteParams): Promise<string> {
    try {
      // Reset/initialize the Yjs document and editor state
      this.ydoc.destroy();
      this.initYjs();
      this.initEditorState();

      if (!this.editorState) {
        throw new Error("Failed to initialize editor state");
      }

      // Generate a client ID
      const clientId = `client-${this.clientID}`;

      // Serialize the initial state
      const yjs_state = Y.encodeStateAsUpdate(this.ydoc);
      const editorJSON = this.editorState.toJSON();
      const content = this.type.toJSON();
      const timestamp = Date.now();

      // Create note content with noteId as the resourceId (will be set after creation)
      const initialContent: NoteContent = {
        content,
        yjs_state,
        editor_state: editorJSON,
        client_id: clientId,
        resource_id: "pending", // Will be updated after we get the note ID
        last_modified: timestamp,
      };

      // Create the note on the server
      const noteId = await sendMessage("addCredential", {
        resourcePayload: JSON.stringify({
          ...initialContent,
          yjs_state: Array.from(yjs_state),
        }),
        folderId: folderId,
        resourceType: "notes",
      });

      // Now update the note with the correct resource_id (same as noteId)
      const updatedContent: NoteContent = {
        ...initialContent,
        resource_id: noteId,
      };

      await sendMessage("updateCredential", {
        id: noteId,
        data: JSON.stringify({
          ...updatedContent,
          yjs_state: Array.from(yjs_state),
        }),
      });

      // Set the current note ID and return it
      this.currentNoteId = noteId;
      return noteId;
    } catch (error) {
      console.error("Error creating note:", error);
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
      schema: this.editorSchema,
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
      const timestamp = Date.now();

      const noteContent: NoteContent = {
        content,
        yjs_state,
        editor_state: editorJSON,
        client_id: `client-${this.clientID}`,
        resource_id: this.currentNoteId, // Use the noteId as resourceId
        last_modified: timestamp,
      };

      await sendMessage("updateCredential", {
        id: this.currentNoteId,
        data: JSON.stringify({
          ...noteContent,
          yjs_state: Array.from(yjs_state),
        }),
      });

      console.log(`Saved note ${this.currentNoteId} successfully`);
    } catch (error) {
      console.error("Error saving note:", error);
      throw error;
    }
  }

  async loadNote(noteId: string) {
    try {
      console.log(`Loading note: ${noteId}`);

      const response = await sendMessage("getCredential", {
        resourceId: noteId,
      });

      if (!response || !response.data) {
        throw new Error("Note not found");
      }

      this.currentNoteId = noteId;
      const noteContent = response.data;

      console.log("Note data loaded:", noteContent);

      // Reset the Yjs document
      this.ydoc.destroy();
      this.initYjs();

      // Apply the saved Yjs state if available
      if (noteContent.yjs_state && noteContent.yjs_state.length > 0) {
        console.log(
          `Applying YJS state with length: ${noteContent.yjs_state.length}`,
        );
        try {
          const yjs_state = new Uint8Array(noteContent.yjs_state);
          Y.applyUpdate(this.ydoc, yjs_state);
          console.log("YJS state applied successfully");

          // Log the YJS document content after applying the update
          console.log("YJS document content after update:", this.type.toJSON());
        } catch (err) {
          console.error("Error applying YJS state:", err);
        }
      } else {
        console.warn("No YJS state to apply");
      }

      // Initialize the editor state AFTER applying the YJS state
      this.initEditorState();

      if (!this.editorState) {
        throw new Error("Failed to initialize editor state");
      }

      console.log("Editor state initialized successfully");

      // Return the document information for the editor component
      return this.getDoc();
    } catch (error) {
      console.error("Error loading note:", error);
      throw error;
    }
  }

  async handleCollaborationUpdate(update: Uint8Array) {
    try {
      if (!this.currentNoteId) {
        console.warn("No current note ID, skipping collaboration update");
        return;
      }

      if (update.length === 0) {
        console.warn("Received empty update");
        return;
      }

      const updateArray = Array.from(update);
      console.log("Sending collaboration update:", updateArray.length, "bytes");

      await emit("sync-update", {
        update: updateArray,
        clientID: this.clientID,
        client_id: `client-${this.clientID}`,
        resource_id: this.currentNoteId,
      });

      // Don't auto-save here, it causes too many saves
      // Let the auto-save interval handle it
    } catch (error) {
      console.error("Error handling collaboration update:", error);
    }
  }

  applyUpdate(update: Uint8Array | number[], sender: number) {
    if (sender === this.clientID) {
      console.log("Ignoring own update");
      return;
    }

    try {
      const updateArray =
        update instanceof Uint8Array ? update : new Uint8Array(update);

      if (updateArray.length === 0) {
        console.warn("Received empty update to apply");
        return;
      }

      console.log("Applying remote update:", updateArray.length, "bytes");
      // Apply update with 'sync' origin to prevent loop
      Y.applyUpdate(this.ydoc, updateArray, "sync");

      console.log("Remote update applied successfully");
    } catch (error) {
      console.error("Error applying update:", error);
    }
  }

  destroy() {
    if (this.ydoc) {
      this.ydoc.destroy();
    }
  }
}

export const notesInstance = new Notes();
