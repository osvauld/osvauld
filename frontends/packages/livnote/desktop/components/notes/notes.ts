import { sendMessage } from "@osvauld/password-manager-common/utils/helper";
import { emit } from "@tauri-apps/api/event";
import { baseKeymap, setBlockType, exitCode } from "prosemirror-commands";
import { keymap } from "prosemirror-keymap";
import { Schema } from "prosemirror-model";
import type { NodeSpec } from "prosemirror-model";
import { schema as basicSchema } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";
import { EditorState } from "prosemirror-state";
import { slashCommandPlugin } from "./slashCommandPlugin";
import { fixedMenuPlugin } from "./fixedMenuPlugin";
import { floatingMenuPlugin } from "./floatingMenuPlugin";
import { clipboardImagePlugin } from "./clipboardImagePlugin";
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
  initProseMirrorDoc,
} from "y-prosemirror";
import { Awareness } from "y-protocols/awareness";
import * as Y from "yjs";
import { dataState } from "../../state";
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
  private pendingYjsState: Uint8Array | null = null;

  constructor() {
    this.clientID = 0;
    this.initSchema();
    this.initYjs();
  }

  updateClientId(clientID: number): void {
    // Update the client ID
    this.clientID = clientID;

  }



  private initSchema(): void {
    // Get the base paragraph node spec from the schema
    const nodes = basicSchema.spec.nodes;
    const baseMarks = basicSchema.spec.marks;

    // Ensure the 'code' mark exists in the basic schema
    const codeMarkSpec = baseMarks.get("code");
    if (!codeMarkSpec) {
      throw new Error("Base schema does not contain a 'code' mark spec.");
    }

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
    const imageSpec: NodeSpec = {
      inline: true,
      attrs: {
        src: {},
        alt: { default: null },
        title: { default: null },
        width: { default: null },
        height: { default: null }
      },
      group: "inline",
      draggable: true,
      parseDOM: [{
        tag: "img[src]",
        getAttrs(dom: HTMLElement) {
          return {
            src: dom.getAttribute("src"),
            alt: dom.getAttribute("alt"),
            title: dom.getAttribute("title"),
            width: dom.getAttribute("width"),
            height: dom.getAttribute("height")
          };
        }
      }],
      toDOM(node) {
        return ["img", node.attrs];
      }
    };


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
      nodes: addListNodes(modifiedNodes, "paragraph block*", "block")
        .addToEnd("image", imageSpec),
      marks: {
        // Define marks explicitly
        strong: {
          parseDOM: [
            { tag: "strong" },
            {
              tag: "span",
              getAttrs: (node: HTMLElement) => node.style.fontWeight != "normal" && null,
            },
          ],
          toDOM() {
            return ["strong", 0];
          },
        },
        em: {
          parseDOM: [{ tag: "i" }, { tag: "em" }, { style: "font-style=italic" }],
          toDOM() {
            return ["em", 0];
          },
        },
        code: codeMarkSpec,
        fontSize: {
          attrs: {
            size: { default: null }
          },
          inclusive: true,
          parseDOM: [{
            style: "font-size",
            getAttrs: (value) => value ? { size: value } : null
          }],
          toDOM(mark) {
            return mark.attrs.size
              ? ["span", { style: `font-size: ${mark.attrs.size}` }, 0]
              : ["span", 0];
          }
        },
        // Add underline mark
        underline: {
          parseDOM: [
            { tag: "u" },
            { style: "text-decoration=underline" }
          ],
          toDOM() {
            return ["u", 0];
          },
        },
        // Add strikethrough mark
        strikethrough: {
          parseDOM: [
            { tag: "s" },
            { style: "text-decoration=line-through" }
          ],
          toDOM() {
            return ["s", 0];
          }
        },
        // Add textColor mark
        textColor: {
          attrs: {
            color: { default: null } // Store the color value
          },
          inclusive: true, // Allow mark to span across nodes
          parseDOM: [{
            style: "color", // Read 'color' style attribute
            getAttrs: (value) => value ? { color: value } : null // Extract color value
          }],
          toDOM(mark) {
            // Render as a span with the color style if color attribute exists
            return mark.attrs.color
              ? ["span", { style: `color: ${mark.attrs.color}` }, 0]
              : ["span", 0];
          }
        },
        // Add link mark (reuse base spec, customize toDOM)
        link: baseMarks.get("link") ? {
          ...baseMarks.get("link")!.spec, // Get base spec
          toDOM(mark) { // Override toDOM to add target="_blank" etc.
            return ["a", { href: mark.attrs.href, title: mark.attrs.title, target: "_blank", rel: "noopener noreferrer" }, 0];
          }
        } : { // Fallback (shouldn't happen)
          attrs: {
            href: {},
            title: { default: null },
          },
          inclusive: false,
          parseDOM: [{
            tag: "a[href]",
            getAttrs(dom: HTMLElement) {
              return {
                href: dom.getAttribute("href"),
                title: dom.getAttribute("title"),
              };
            },
          }],
          toDOM(mark) {
            return ["a", { href: mark.attrs.href, title: mark.attrs.title, target: "_blank", rel: "noopener noreferrer" }, 0];
          },
        },
      }
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

      /* Inline code style */
      .ProseMirror code {
        background-color: #3a3b44; /* Slightly dark background */
        padding: 0.1em 0.4em;
        border-radius: 4px;
        font-family: monospace; /* Explicitly set monospace font */
        font-size: 0.9em; /* Slightly smaller font size */
      }

      /* Reset inline code styles when inside a pre (code block) */
      .ProseMirror pre code {
        background-color: initial; /* Reset background */
        padding: initial; /* Reset padding */
        border-radius: initial; /* Reset border-radius */
        font-size: inherit; /* Inherit font size from pre */
        /* font-family is likely already monospace via pre or global styles */
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
        const result = initProseMirrorDoc(this.type, this.editorSchema);
        prosemirrorDoc = result.doc;
        console.log("Successfully created ProseMirror doc from YJS content");

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
          clipboardImagePlugin(),
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
        client_id: this.clientID.toString(),
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

      await emit("note-change", noteId);
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
        client_id: this.clientID.toString(),
        resource_id: this.currentNoteId,
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
      emit('resource-update-complete', { id: this.currentNoteId });

      console.log(`Saved note ${this.currentNoteId} successfully`);
    } catch (error) {
      console.error("Error saving note:", error);
      throw error;
    }
  }

  async loadNote(noteId: string): Promise<EditorDocumentState> {
    console.time('notes-loadNote-total');
    try {
      console.log(`Loading note: ${noteId}`);

      // Measure data fetch time
      console.time('notes-fetchData');
      const response = dataState.getNoteById(noteId);
      console.timeEnd('notes-fetchData');

      if (!response || !response.data) {
        throw new Error("Note not found");
      }

      this.currentNoteId = noteId;
      const noteContent = response.data;

      // Measure Yjs document initialization
      console.time('notes-resetYdoc');
      this.ydoc.destroy();
      this.initYjs();
      console.timeEnd('notes-resetYdoc');
      if (noteContent.yjs_state && noteContent.yjs_state.length > 0) {
        this.pendingYjsState = new Uint8Array(noteContent.yjs_state);

      }

      // Measure editor state initialization
      console.time('notes-initEditorState');
      this.initEditorState();
      console.timeEnd('notes-initEditorState');

      return this.getDoc();
    } catch (error) {
      console.error("Error loading note:", error);
      throw error;
    } finally {
      console.timeEnd('notes-loadNote-total');
    }
  }

  applyPendingYjsState(view: any | null): void {
    if (!this.pendingYjsState) {
      console.log("No pending YJS state to apply");
      return;
    }

    console.log(`Applying pending YJS state with length: ${this.pendingYjsState.length}`);

    try {
      // Apply the YJS state
      console.time('applyYjsState');
      Y.applyUpdate(this.ydoc, this.pendingYjsState, 'sync');
      console.timeEnd('applyYjsState');

      console.log("YJS state applied successfully");

      // Update the view if provided
      if (view) {
        const tr = view.state.tr;
        view.dispatch(tr);
      }
    } catch (err) {
      console.error("Error applying pending YJS state:", err);
    } finally {
      // Clear the pending state
      this.pendingYjsState = null;
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
        client_id: this.clientID.toString(),
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
