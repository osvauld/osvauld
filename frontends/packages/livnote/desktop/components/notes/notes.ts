import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
import { emit } from "@tauri-apps/api/event";
import { baseKeymap, setBlockType, exitCode } from "prosemirror-commands";
import { keymap } from "prosemirror-keymap";
import { Schema } from "prosemirror-model";
import type { NodeSpec } from "prosemirror-model";
import { schema } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";
import { EditorState } from "prosemirror-state";
import { slashCommandPlugin } from "./slashCommandPlugin";
import { fixedMenuPlugin } from "./fixedMenuPlugin";
import { floatingMenuPlugin } from "./floatingMenuPlugin";
import {
  wrapInList,
  splitListItem,
  liftListItem,
  sinkListItem,
} from "prosemirror-schema-list";
import { dropCursor } from "prosemirror-dropcursor";
import { gapCursor } from "prosemirror-gapcursor";
import { history } from "prosemirror-history";
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
import type {
  NoteContent,
  CreateNoteParams,
  UserInfo,
  EditorDocumentState,
  NoteResponse,
  CollaborationUpdateEvent
} from "../../types/notes.types";

// Type definitions for notes, states and other components

/**
 * Note content structure for storage, retrieval and synchronization
 */

/**
 * Parameters for note creation operations
 */

/**
 * User information for collaboration awareness
 */

/**
 * Editor document state for collaboration
 */

/**
 * Response structure from the server for note operations
 */

/**
 * Collaboration update event data
 */

export class Notes {
  private ydoc!: Y.Doc;
  private type!: Y.XmlFragment;
  private awareness!: Awareness;
  private clientID: number;
  private currentNoteId: string | null = null;
  private editorState: EditorState | null = null;
  private editorSchema!: Schema;

  constructor() {
    this.clientID = Math.floor(Math.random() * 0xffffffff);
    this.initSchema();
    this.initYjs();
  }

  private initSchema(): void {
    // Get the base paragraph node spec from the schema
    const nodes = schema.spec.nodes;

    // Helper function to add indent and align attributes to a node spec
    const addIndentAndAlignAttrs = (nodeSpec: NodeSpec): NodeSpec => ({
      ...nodeSpec,
      attrs: {
        ...nodeSpec.attrs,
        align: { default: null },
        indent: { default: null },
      },
      parseDOM: [
        {
          tag: nodeSpec.parseDOM?.[0]?.tag || "p",
          getAttrs(dom: HTMLElement) {
            const existingAttrs = nodeSpec.parseDOM?.[0]?.getAttrs ?
              nodeSpec.parseDOM[0].getAttrs(dom) :
              {};
            return {
              ...existingAttrs,
              align: dom.style.textAlign || null,
              indent: dom.hasAttribute("data-indent")
                ? parseInt(dom.getAttribute("data-indent") || "0", 10)
                : null,
            };
          },
        },
      ],
      toDOM(node) {
        const attrs: { [key: string]: any } = {};

        if (node.attrs.align) {
          attrs.style = `text-align: ${node.attrs.align}`;
        }

        if (node.attrs.indent && node.attrs.indent > 0) {
          attrs["data-indent"] = node.attrs.indent;
        }

        return [nodeSpec.parseDOM?.[0]?.tag || "p", attrs, 0] as [string, Object, number];
      },
    });

    // Get the heading node spec and modify it
    const headingSpec = nodes.get("heading");
    if (!headingSpec) {
      throw new Error("Heading node spec not found in schema");
    }

    const modifiedHeadingSpec: NodeSpec = {
      ...headingSpec,
      attrs: {
        ...headingSpec.attrs,
        align: { default: null },
        indent: { default: null },
      },
      parseDOM: (headingSpec.parseDOM || []).map(spec => ({
        tag: spec.tag,
        getAttrs(dom: HTMLElement) {
          const existingAttrs = spec.getAttrs ? spec.getAttrs(dom) : {};
          return {
            ...existingAttrs,
            align: dom.style.textAlign || null,
            indent: dom.hasAttribute("data-indent")
              ? parseInt(dom.getAttribute("data-indent") || "0", 10)
              : null,
          };
        }
      })),
      toDOM(node) {
        const attrs: { [key: string]: any } = {};

        if (node.attrs.align) {
          attrs.style = `text-align: ${node.attrs.align}`;
        }

        if (node.attrs.indent && node.attrs.indent > 0) {
          attrs["data-indent"] = node.attrs.indent;
        }

        return [`h${node.attrs.level}`, attrs, 0] as [string, Object, number];
      }
    };

    // Modify both paragraph and heading nodes
    const paragraphSpec = nodes.get("paragraph");
    if (!paragraphSpec) {
      throw new Error("Paragraph node spec not found in schema");
    }

    const modifiedNodes = nodes
      .update("paragraph", addIndentAndAlignAttrs(paragraphSpec))
      .update("heading", modifiedHeadingSpec);

    // Add list nodes to our modified nodes
    this.editorSchema = new Schema({
      nodes: addListNodes(modifiedNodes, "paragraph block*", "block"),
      marks: schema.spec.marks,
    });

    // Add CSS for indentation and alignment
    this.addCustomStyles();
  }

  // Helper method to add the required CSS
  private addCustomStyles(): void {
    const styleElement = document.createElement("style");
    styleElement.textContent = `
      /* Text alignment styles */
      .ProseMirror [style*="text-align: center"] {
        text-align: center;
      }
      
      .ProseMirror [style*="text-align: right"] {
        text-align: right;
      }
      
      /* Indentation styles for paragraphs and headings */
      .ProseMirror p[data-indent="1"],
      .ProseMirror h1[data-indent="1"],
      .ProseMirror h2[data-indent="1"],
      .ProseMirror h3[data-indent="1"],
      .ProseMirror h4[data-indent="1"],
      .ProseMirror h5[data-indent="1"],
      .ProseMirror h6[data-indent="1"] {
        margin-left: 2em;
      }
      
      .ProseMirror p[data-indent="2"],
      .ProseMirror h1[data-indent="2"],
      .ProseMirror h2[data-indent="2"],
      .ProseMirror h3[data-indent="2"],
      .ProseMirror h4[data-indent="2"],
      .ProseMirror h5[data-indent="2"],
      .ProseMirror h6[data-indent="2"] {
        margin-left: 4em;
      }
      
      .ProseMirror p[data-indent="3"],
      .ProseMirror h1[data-indent="3"],
      .ProseMirror h2[data-indent="3"],
      .ProseMirror h3[data-indent="3"],
      .ProseMirror h4[data-indent="3"],
      .ProseMirror h5[data-indent="3"],
      .ProseMirror h6[data-indent="3"] {
        margin-left: 6em;
      }
    `;
    document.head.appendChild(styleElement);
  }

  private initYjs(): void {
    this.ydoc = new Y.Doc();
    this.type = this.ydoc.getXmlFragment("prosemirror");
    this.awareness = new Awareness(this.ydoc);

    // Set up observer for document updates with origin tracking
    this.ydoc.on("update", (update: Uint8Array, origin: any) => {
      // Only handle updates that originated locally (not from sync)
      if (origin !== "sync" && origin !== "loading") {
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
      } as UserInfo,
    });
  }

  public updateUserInfo(name: string, color: string): void {
    this.awareness.setLocalStateField('user', {
      name,
      color,
      id: this.clientID
    } as UserInfo);
  }

  private createBasicCustomCursor(user: UserInfo): HTMLElement {
    const cursor = document.createElement('span');
    cursor.style.borderLeft = `2px solid ${user.color}`;
    cursor.style.marginLeft = '-1px';
    cursor.style.paddingLeft = '1px';
    cursor.style.position = 'relative';
    cursor.style.height = '1.2em';
    cursor.style.display = 'inline-block';
    return cursor;
  }

  private initEditorState(): void {
    const listKeymap = keymap({
      Enter: splitListItem(this.editorSchema.nodes.list_item),
      Tab: sinkListItem(this.editorSchema.nodes.list_item),
      "Shift-Tab": liftListItem(this.editorSchema.nodes.list_item),
      "Ctrl-Shift-8": wrapInList(this.editorSchema.nodes.bullet_list),
      "Ctrl-Shift-9": wrapInList(this.editorSchema.nodes.ordered_list),
    });

    const codeBlockKeymap = keymap({
      "Shift-Enter": exitCode, // Use Shift+Enter to exit the code block
      Enter: (state, dispatch) => {
        // Basic Enter key just creates a new line within the code block
        if (dispatch) {
          const { $from, $to } = state.selection;
          dispatch(
            state.tr
              .replaceSelectionWith(state.schema.text("\n"))
              .scrollIntoView(),
          );
        }
        return true;
      },
    });

    // Add hard break keymap for Shift+Enter in regular text
    const hardBreakKeymap = keymap({
      "Shift-Enter": (state, dispatch) => {
        const { selection } = state;
        const { $from, $to } = selection;

        // Don't handle if we're in a code block (codeBlockKeymap handles it)
        if ($from.parent.type.name === "code_block") return false;

        // Insert a hard break at the current position
        if (dispatch) {
          const hardBreak = state.schema.nodes.hard_break.create();
          dispatch(state.tr.replaceSelectionWith(hardBreak).scrollIntoView());
        }
        return true;
      }
    });

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
        prosemirrorDoc = this.editorSchema.node("doc", null, [
          this.editorSchema.node("paragraph", null, [])
        ]);
        console.log("Created empty ProseMirror doc instead");
      }

      // Create the editor state with the document
      const doc = (prosemirrorDoc as any).doc || prosemirrorDoc;
      this.editorState = EditorState.create({
        schema: this.editorSchema,
        doc: doc,
        plugins: [
          slashCommandPlugin(this.editorSchema),
          listKeymap,
          hardBreakKeymap,
          keymap(baseKeymap),
          codeBlockKeymap,
          syncPlugin,
          dropCursor(),
          gapCursor(),
          history(),
          fixedMenuPlugin(this.editorSchema),
          floatingMenuPlugin(this.editorSchema),
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
          keymap(baseKeymap),
          listKeymap,
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

  getDoc(): EditorDocumentState {
    return {
      ydoc: this.ydoc,
      type: this.type,
      awareness: this.awareness,
      clientID: this.clientID,
      editorState: this.editorState,
      schema: this.editorSchema,
    };
  }

  updateEditorState(newState: EditorState): void {
    this.editorState = newState;
  }

  async saveNote(title = "Untitled note"): Promise<void> {
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
        title,
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

  async loadNote(noteId: string): Promise<EditorDocumentState> {
    try {
      console.log(`Loading note: ${noteId}`);

      const response: NoteResponse = await sendMessage("getCredential", {
        resourceId: noteId,
      });

      if (!response || !response.data) {
        throw new Error("Note not found");
      }

      this.currentNoteId = noteId;
      const noteContent = response.data;

      // console.log("Note data loaded:", noteContent);

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
          Y.applyUpdate(this.ydoc, yjs_state, "loading");
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

  async handleCollaborationUpdate(update: Uint8Array): Promise<void> {
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
      } as CollaborationUpdateEvent);
    } catch (error) {
      console.error("Error handling collaboration update:", error);
    }
  }

  applyUpdate(update: Uint8Array | number[], sender: number): void {
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

  destroy(): void {
    if (this.ydoc) {
      this.ydoc.destroy();
    }
  }
}

export const notesInstance = new Notes();
