import { sendMessage } from "../../utils/helper";
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
import { pasteHandlerPlugin } from "./pasteHandlerPlugin";
import { encodeAwarenessUpdate, applyAwarenessUpdate } from 'y-protocols/awareness';
import {
  wrapInList,
  splitListItem,
  liftListItem,
  sinkListItem,
} from "prosemirror-schema-list";
import { EditorView } from "prosemirror-view";
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
  UserInfo,
  EditorDocumentState,
  CommentThread,
  CommentPosition,
  Collaborator,
  ImageAsset,
  ImageMetadata
} from "../../types/notes.types";
import { markdownShortcutsPlugin } from "./markdownShortcutsPlugin";
import { CommentsService } from "./commentsService";
import { ImageStorageService } from "./imageStorage";
import { imageNodeViewPlugin } from "./imageNodeViewPlugin";

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
  private imageDoc: Y.Doc;
  private type!: Y.XmlFragment;
  private commentsMap!: Y.Map<CommentThread>;
  private imagesMap!: Y.Map<ImageMetadata>;
  private commentsService!: CommentsService;
  private imageStorage!: ImageStorageService;
  private awareness!: Awareness;
  private clientID: number;
  public currentNoteId: string | null = null;
  private editorState: EditorState | null = null;
  private editorSchema!: Schema;
  private pendingYjsState: Uint8Array | null = null;
  private pendingImageState: Uint8Array | null = null;
  private metadata!: Y.Map<any>;
  private editorView: EditorView | null = null;
  private imageLoadPromise: Promise<void> | null = null;
  private _imagesLoaded: boolean = false;
  private currentAssets: ImageAsset[] = [];
  constructor() {
    this.clientID = 0;
    this.initSchema();
  }

  updateClientId(clientID: number): void {
    // Update the client ID
    this.clientID = clientID;
    if (this.ydoc) {
      this.ydoc.destroy();
    }
    this.initYjs();

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
      nodes: addListNodes(modifiedNodes, "paragraph block*", "block")
        .addToEnd("image", imageSpec),
      marks: {
        // Define marks explicitly
        strong: {
          parseDOM: [
            { tag: "strong" },
            { tag: "b" },
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
            { tag: "strike" },
            { tag: "del" },
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
        // Add fontFamily mark
        fontFamily: {
          attrs: {
            family: { default: null } // Store the font family value
          },
          inclusive: true, // Allow mark to span across nodes
          parseDOM: [{
            style: "font-family", // Read 'font-family' style attribute
            getAttrs: (value) => value ? { family: value } : null // Extract font family value
          }],
          toDOM(mark) {
            // Render as a span with the font-family style if family attribute exists
            return mark.attrs.family
              ? ["span", { style: `font-family: ${mark.attrs.family}` }, 0]
              : ["span", 0];
          }
        },
        // Add link mark explicitly, not relying on baseMarks.get("link") for the core definition
        link: {
          attrs: {
            href: { default: null },
            title: { default: null }
          },
          inclusive: false,
          excludes: "underline",
          parseDOM: [{
            tag: "a[href]",
            getAttrs(dom: HTMLElement) {
              const href = dom.getAttribute("href");
              const dataMceHref = dom.getAttribute("data-mce-href");
              let finalHref = href;

              if (!href || href.trim() === "" || href.trim() === "#") {
                if (dataMceHref && dataMceHref.trim() !== "") {
                  finalHref = dataMceHref;
                }
              }

              if (!finalHref || finalHref.trim() === "") {
                return false;
              }

              return {
                href: finalHref,
                title: dom.getAttribute("title") || dom.textContent?.trim() || "",
              };
            },
          }],
          toDOM(mark) {
            // The `attrs` definition ensures `href` and `title` have defaults (null).
            // `getAttrs` returns false if a valid href isn't found, preventing mark creation.
            // So, if the mark exists, `mark.attrs.href` should be a valid string.
            return ["a", {
              href: mark.attrs.href,
              title: mark.attrs.title,
              target: "_blank",
              rel: "noopener noreferrer"
            }, 0];
          }
        },
        // Add comment mark for collaborative commenting
        comment: {
          attrs: {
            threadId: {},
            commentIds: { default: [] },
            resolved: { default: false },
            author: { default: null }
          },
          inclusive: false,
          excludes: "", // Allow stacking with other marks
          parseDOM: [{
            tag: "span[data-livnote-comment]",
            getAttrs(dom: HTMLElement) {
              return {
                threadId: dom.getAttribute("data-livnote-comment"),
                commentIds: JSON.parse(dom.getAttribute("data-livnote-comment-ids") || "[]"),
                resolved: dom.getAttribute("data-livnote-resolved") === "true",
                author: dom.getAttribute("data-livnote-author") || null
              };
            }
          }],
          toDOM(mark) {
            const { threadId, commentIds, resolved, author } = mark.attrs;
            return ["span", {
              "data-livnote-comment": threadId,
              "data-livnote-comment-ids": JSON.stringify(commentIds),
              "data-livnote-comment-count": commentIds.length.toString(),
              "data-livnote-resolved": resolved ? "true" : "false",
              "data-livnote-author": author || "",
              "data-livnote-internal": "true", // Mark as internal
              class: `livnote-comment-highlight ${resolved ? 'resolved' : 'active'}`,
              style: resolved
                ? "border-bottom: 2px solid #888; background: rgba(136, 136, 136, 0.1);"
                : "border-bottom: 2px solid #ffd700; background: rgba(255, 215, 0, 0.1);"
            }, 0];
          }
        }
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

      /* Hide ProseMirror separator and trailing break elements that cause line height issues */
      .ProseMirror-separator {
        display: none !important;
      }
      .image-node-container {
        display: inline-block;
        position: relative;
        margin: 0.5em 0;
      }

      .image-placeholder {
        display: flex;
        align-items: center;
        justify-content: center;
        background-color: #2a2b35;
        border-radius: 4px;
        color: #85889C;
        font-size: 14px;
      }

      .ProseMirror-selectednode {
        outline: 2px solid #4094ef;
        outline-offset: 2px;
      }
    `;
    document.head.appendChild(styleElement);
  }

  setEditorView(view: EditorView | null): void {
    this.editorView = view;
  }
  private initYjs(): void {
    this.ydoc = new Y.Doc();
    this.imageDoc = new Y.Doc();
    this.type = this.ydoc.getXmlFragment("prosemirror");
    this.commentsMap = this.ydoc.getMap("comments");
    this.ydoc.clientID = this.clientID;
    this.awareness = new Awareness(this.ydoc);
    this.metadata = this.ydoc.getMap("metadata");
    this.imagesMap = this.imageDoc.getMap("images");
    // Initialize comments service
    this.commentsService = new CommentsService(this.commentsMap);
    this.imageStorage = new ImageStorageService(this.imagesMap, this.clientID);
    // Set up observer for document updates with origin tracking
    this.ydoc.on("update", (update: Uint8Array, origin: any) => {
      // Only handle updates that originated locally (not from sync)
      if (origin !== "sync" && origin !== "loading") {
        void this.handleCollaborationUpdate(update, "main");
      }
    });
    this.imageDoc.on("update", (update: Uint8Array, origin: any) => {
      if (origin !== "sync" && origin !== "loading") {
        void this.handleCollaborationUpdate(update, 'images');
      }
    });

    this.awareness.on('change', (changes: { added: number[], updated: number[], removed: number[] }, origin: string) => {
      if (origin === 'local') {
        void this.handleAwarenessUpdate(changes);
      }
      this.syncCollaboratorsToDataState();
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
    const username = dataState.userDetails?.username || `User ${this.clientID}`;
    // Set enhanced local user state
    this.awareness.setLocalState({
      user: {
        name: username,
        color: userColor,
        id: this.clientID,
      } as UserInfo,
    });

    // Set current user for comments service
    this.commentsService.setCurrentUser({
      name: username,
      color: userColor,
      id: this.clientID,
    });
  }
  private syncCollaboratorsToDataState(): void {
    if (!this.awareness) return;

    const awarenessStates = this.awareness.getStates();
    const currentCollaborators: Collaborator[] = [];

    awarenessStates.forEach((state, clientId) => {
      // Skip our own client
      if (clientId === this.clientID) return;

      if (state && state.user) {
        currentCollaborators.push({
          id: clientId.toString(),
          name: state.user.name || `User ${clientId}`,
          color: state.user.color || "#85889C",
          clientId: clientId,
        });
      }
    });

    // Update the centralized state
    dataState.updateCollaborators(currentCollaborators);
  }


  public updateUserInfo(name: string, color: string): void {
    this.awareness.setLocalStateField('user', {
      name,
      color,
      id: this.clientID
    } as UserInfo);
  }

  getCurrentTitle(): string {
    return this.metadata.get("title") || "Untitled Note";
  }

  setTitle(title: string): void {
    this.metadata.set("title", title);
  }

  getImageStorage(): ImageStorageService {
    return this.imageStorage;
  }

  private createBasicCustomCursor(user: UserInfo): HTMLElement {
    const cursor = document.createElement('span');
    cursor.style.borderLeft = `2px solid ${user.color}`;
    cursor.style.marginLeft = '-1px';
    cursor.style.paddingLeft = '1px';
    cursor.style.position = 'relative';
    cursor.style.height = '1.2em';
    cursor.style.display = 'inline-block';
    const banner = document.createElement('div');
    banner.textContent = user.name;
    banner.style.position = 'absolute';
    banner.style.top = '-1.8em';
    banner.style.left = '-1px';
    banner.style.fontSize = '12px';
    banner.style.backgroundColor = user.color;
    banner.style.fontFamily = '"Inter", "Segoe UI", sans-serif';
    banner.style.fontWeight = '500';
    banner.style.lineHeight = 'normal';
    banner.style.userSelect = 'none';
    banner.style.color = 'white';
    banner.style.padding = '3px 8px';
    banner.style.borderRadius = '4px';
    banner.style.whiteSpace = 'nowrap';
    banner.style.boxShadow = '0 1px 3px rgba(0, 0, 0, 0.2)';
    banner.style.zIndex = '21';
    cursor.appendChild(banner);
    return cursor;
  }

  private initEditorState(): void {
    const initStart = performance.now();
    console.log(`[NOTES] Starting initEditorState at ${initStart}`);

    // Time keymap creation
    const keymapStart = performance.now();
    const listKeymap = keymap({
      Enter: splitListItem(this.editorSchema.nodes.list_item),
      Tab: sinkListItem(this.editorSchema.nodes.list_item),
      "Shift-Tab": liftListItem(this.editorSchema.nodes.list_item),
      "Ctrl-Shift-8": wrapInList(this.editorSchema.nodes.bullet_list),
      "Ctrl-Shift-9": wrapInList(this.editorSchema.nodes.ordered_list),
    });

    const codeBlockKeymap = keymap({
      "Shift-Enter": exitCode,
      Enter: (state, dispatch) => {
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

    const hardBreakKeymap = keymap({
      "Shift-Enter": (state, dispatch) => {
        const { selection } = state;
        const { $from, $to } = selection;

        if ($from.parent.type.name === "code_block") return false;

        if (dispatch) {
          const hardBreak = state.schema.nodes.hard_break.create();
          dispatch(state.tr.replaceSelectionWith(hardBreak).scrollIntoView());
        }
        return true;
      }
    });
    const keymapTime = performance.now() - keymapStart;
    console.log(`[NOTES] Keymap creation took ${keymapTime.toFixed(2)}ms`);

    try {
      // Time sync plugin creation
      const syncPluginStart = performance.now();
      const syncPlugin = ySyncPlugin(this.type);
      const syncPluginTime = performance.now() - syncPluginStart;
      console.log(`[NOTES] Sync plugin creation took ${syncPluginTime.toFixed(2)}ms`);

      // Time ProseMirror document initialization - THIS IS LIKELY THE BOTTLENECK
      const docInitStart = performance.now();
      let prosemirrorDoc;
      try {
        const result = initProseMirrorDoc(this.type, this.editorSchema);
        prosemirrorDoc = result.doc;
        const docInitTime = performance.now() - docInitStart;
        console.log(`[NOTES] ProseMirror doc init took ${docInitTime.toFixed(2)}ms`);

        // Check document size
        const docSize = JSON.stringify(prosemirrorDoc.toJSON()).length;
        console.log(`[NOTES] ProseMirror document size: ${(docSize / 1024 / 1024).toFixed(2)}MB`);

        // Time document processing for headings
        if (prosemirrorDoc.childCount === 1 && prosemirrorDoc.firstChild && prosemirrorDoc.firstChild.type.name === "heading") {
          const headingProcessStart = performance.now();
          const headingContent = prosemirrorDoc.firstChild.textContent;
          const paragraphTexts = headingContent.split(/\n\n|\r\n\r\n/);
          const paragraphNodes = paragraphTexts.map((text) =>
            this.editorSchema.node("paragraph", {}, [
              this.editorSchema.text(text.trim()),
            ]),
          );

          if (paragraphNodes.length === 0) {
            paragraphNodes.push(this.editorSchema.node("paragraph", {}, []));
          }

          prosemirrorDoc = this.editorSchema.node("doc", {}, paragraphNodes);
          const headingProcessTime = performance.now() - headingProcessStart;
          console.log(`[NOTES] Heading processing took ${headingProcessTime.toFixed(2)}ms`);
        }

      } catch (err) {
        console.error("Error creating ProseMirror doc from YJS:", err);
        prosemirrorDoc = this.editorSchema.node("doc", null, [
          this.editorSchema.node("paragraph", null, [])
        ]);
      }

      // Time final editor state creation
      const stateCreateStart = performance.now();
      const doc = (prosemirrorDoc as any).doc || prosemirrorDoc;
      this.editorState = EditorState.create({
        schema: this.editorSchema,
        doc: doc,
        plugins: [
          pasteHandlerPlugin(this.imageStorage),
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
          markdownShortcutsPlugin(this.editorSchema),
          keymap({
            "Mod-z": undo,
            "Mod-y": redo,
            "Mod-Shift-z": redo,
          }),
          imageNodeViewPlugin(this.imageStorage),
        ],
      });
      const stateCreateTime = performance.now() - stateCreateStart;
      console.log(`[NOTES] Editor state creation took ${stateCreateTime.toFixed(2)}ms`);

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

    const totalInitTime = performance.now() - initStart;
    console.log(`[NOTES] Total initEditorState took ${totalInitTime.toFixed(2)}ms`);
  }
  /**
   * Creates a new note with initialized state in a single operation
   * @param params Parameters for note creation
   * @returns The ID of the created note
   */
  createDefaultNote(): NoteContent {
    try {
      // Reset/initialize the Yjs document and editor state
      this.ydoc.destroy();
      this.imageDoc.destroy();
      this.initYjs();
      this.initEditorState();
      this.setTitle("Untitled Note");

      // Initialize empty assets array
      this.currentAssets = [];
      this.imageStorage.setAssetsArray(this.currentAssets);

      if (!this.editorState) {
        throw new Error("Failed to initialize editor state");
      }

      // Serialize the initial state
      const yjs_state = Y.encodeStateAsUpdate(this.ydoc);
      const image_state = Y.encodeStateAsUpdate(this.imageDoc); // Only metadata now
      const editorJSON = this.editorState.toJSON();
      const content = this.type.toJSON();
      const timestamp = Date.now();

      // Create note content with empty assets array
      const initialContent: NoteContent = {
        content,
        yjs_state,
        image_state,
        assets: [], // Start with empty assets
        editor_state: editorJSON,
        client_id: this.clientID.toString(),
        last_modified: timestamp,
        title: this.getCurrentTitle(),
      };

      return initialContent;
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


  updateTitle(newTitle: string): void {
    if (!newTitle.trim()) {
      newTitle = "Untitled Note";
    }
    this.setTitle(newTitle);
  }


  async saveNote(title?: string): Promise<void> {
    if (!this.currentNoteId || !this.editorState) {
      console.error("No note is currently active or editor state is missing");
      return;
    }

    try {
      const yjs_state = Y.encodeStateAsUpdate(this.ydoc);
      const image_state = Y.encodeStateAsUpdate(this.imageDoc); // Only metadata
      const editorJSON = this.editorState.toJSON();
      const content = this.type.toJSON();
      const timestamp = Date.now();

      if (title !== undefined) {
        this.setTitle(title);
      }

      // Include current assets in the save
      const noteContent: NoteContent = {
        content,
        yjs_state: Array.from(yjs_state),
        image_state: Array.from(image_state), // Metadata only
        assets: this.currentAssets, // Include all assets
        editor_state: editorJSON,
        client_id: this.clientID.toString(),
        last_modified: timestamp,
        title: this.getCurrentTitle(),
      };

      const saveSize = JSON.stringify(noteContent).length;
      console.log(`[NOTES] Saving note with total size: ${(saveSize / 1024 / 1024).toFixed(2)}MB`);
      console.log(`[NOTES] Assets count: ${this.currentAssets.length}`);

      await sendMessage("updateCredential", {
        id: this.currentNoteId,
        data: JSON.stringify(noteContent),
      });

      await emit('resource-update-complete', { id: this.currentNoteId });
    } catch (error) {
      console.error("Error saving note:", error);
      throw error;
    }
  }
  /**
     * Get current assets for external access
     */
  getCurrentAssets(): ImageAsset[] {
    return [...this.currentAssets];
  }
  async loadNote(): Promise<EditorDocumentState> {
    const loadStartTime = performance.now();
    console.log(`[NOTES] Starting loadNote at ${loadStartTime}`);

    try {
      const response = dataState.currentNote;

      if (!response || !response.data) {
        throw new Error("Note not found");
      }

      if (dataState.currentNote) {
        this.currentNoteId = dataState.currentNote?.id;
      }

      const noteContent = response.data;

      // Check content size (excluding assets for now)
      const contentWithoutAssets = { ...noteContent };
      delete contentWithoutAssets.assets;
      const contentSize = JSON.stringify(contentWithoutAssets).length;
      console.log(`[NOTES] Note content size (without assets): ${(contentSize / 1024).toFixed(2)}KB`);

      // Log assets info separately
      if (noteContent.assets) {
        const assetsSize = JSON.stringify(noteContent.assets).length;
        console.log(`[NOTES] Assets size: ${(assetsSize / 1024 / 1024).toFixed(2)}MB (${noteContent.assets.length} assets)`);
      }

      // Time YJS document destruction
      const destroyStart = performance.now();
      this.ydoc.destroy();
      this.imageDoc.destroy();
      this.imageLoadPromise = null;
      const destroyTime = performance.now() - destroyStart;
      console.log(`[NOTES] YJS destroy took ${destroyTime.toFixed(2)}ms`);

      // Time YJS initialization
      const initYjsStart = performance.now();
      this.initYjs();
      const initYjsTime = performance.now() - initYjsStart;
      console.log(`[NOTES] initYjs took ${initYjsTime.toFixed(2)}ms`);

      // Set up assets array BEFORE applying YJS state
      const assetsStart = performance.now();
      this.currentAssets = noteContent.assets || [];
      this.imageStorage.setAssetsArray(this.currentAssets);
      const assetsTime = performance.now() - assetsStart;
      console.log(`[NOTES] Assets setup took ${assetsTime.toFixed(2)}ms`);

      // Time YJS state preparation (metadata only now)
      if (noteContent.yjs_state && noteContent.yjs_state.length > 0) {
        const yjsStateStart = performance.now();
        this.pendingYjsState = new Uint8Array(noteContent.yjs_state);
        const yjsStateTime = performance.now() - yjsStateStart;
        console.log(`[NOTES] YJS state preparation took ${yjsStateTime.toFixed(2)}ms`);
        console.log(`[NOTES] YJS state size: ${this.pendingYjsState.length} bytes`);
      }

      // Time editor state initialization
      const initEditorStart = performance.now();
      this.initEditorState();
      const initEditorTime = performance.now() - initEditorStart;
      console.log(`[NOTES] initEditorState took ${initEditorTime.toFixed(2)}ms`);

      // Handle image metadata state (much smaller now)
      if (noteContent.image_state && noteContent.image_state.length > 0) {
        console.log(`[NOTES] Image metadata state size: ${noteContent.image_state.length} bytes`);
        this.pendingImageState = new Uint8Array(noteContent.image_state);
      }

      // Start lazy image loading (now just loads metadata)
      this.startLazyImageLoad();

      const totalTime = performance.now() - loadStartTime;
      console.log(`[NOTES] Total loadNote took ${totalTime.toFixed(2)}ms`);

      return this.getDoc();
    } catch (error) {
      console.error("Error loading note:", error);
      throw error;
    }
  }

  private startLazyImageLoad(): void {
    if (!this.pendingImageState) {
      console.log("[NOTES] No pending image metadata state to load");
      this._imagesLoaded = true;
      return;
    }

    this._imagesLoaded = false;

    // Load image metadata after a short delay
    this.imageLoadPromise = new Promise((resolve) => {
      setTimeout(async () => {
        try {
          const imageLoadStart = performance.now();
          console.log("[NOTES] Starting lazy image metadata load");

          // Apply the image metadata state to the image document
          Y.applyUpdate(this.imageDoc, this.pendingImageState!, 'loading');

          // Clear the pending state
          this.pendingImageState = null;

          const imageLoadTime = performance.now() - imageLoadStart;
          console.log(`[NOTES] Image metadata loaded in ${imageLoadTime.toFixed(2)}ms`);
          this._imagesLoaded = true;

          // Notify image node views that metadata is now available
          this.notifyImageNodesLoaded();

          resolve();
        } catch (error) {
          console.error("Error loading image metadata:", error);
          resolve(); // Resolve anyway to not block
        }
      }, 50); // Reduced delay since metadata is much smaller
    });
  }
  private notifyImageNodesLoaded(): void {
    // Dispatch a custom event that image node views can listen to
    document.dispatchEvent(new CustomEvent('images-loaded', {
      detail: { noteId: this.currentNoteId }
    }));
  }

  applyPendingYjsState(view: any | null): void {
    if (!this.pendingYjsState) {
      return;
    }

    try {
      // Apply the YJS state
      Y.applyUpdate(this.ydoc, this.pendingYjsState, 'sync');

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



  async handleCollaborationUpdate(update: Uint8Array, docType: 'main' | 'images'): Promise<void> {
    try {
      if (!this.currentNoteId) {
        console.warn("No current note ID, skipping collaboration update");
        return;
      }

      const updateArray = Array.from(update);

      await emit("sync-update", {
        update: updateArray,
        clientID: this.clientID,
        resource_id: this.currentNoteId,
        doc_type: docType, // Specify which document this update is for
      });
    } catch (error) {
      console.error("Error handling collaboration update:", error);
    }
  }

  private async handleAwarenessUpdate(changes: { added: number[], updated: number[], removed: number[] }): Promise<void> {
    try {
      if (!this.currentNoteId) {
        console.warn("No current note ID, skipping awareness update");
        return;
      }

      // Get all client IDs that changed
      const clients = [...changes.added, ...changes.updated, ...changes.removed];
      if (clients.length === 0) return;

      // Encode awareness update using YJS awareness protocol
      const update = encodeAwarenessUpdate(this.awareness, clients);

      await emit("awareness-update", {
        update: Array.from(update),
        clientID: this.clientID,           // Keep only this one (number)
        resource_id: this.currentNoteId,
        changes
      });
    } catch (error) {
      console.error("Error handling awareness update:", error);
    }
  }

  applyUpdate(update: Uint8Array | number[], sender: number, docType: 'main' | 'images' = 'main'): void {
    if (sender === this.clientID) {
      return;
    }

    try {
      const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);

      if (updateArray.length === 0) {
        console.warn("Received empty update to apply");
        return;
      }

      // Apply to the appropriate document
      const targetDoc = docType === 'images' ? this.imageDoc : this.ydoc;
      Y.applyUpdate(targetDoc, updateArray, "sync");
    } catch (error) {
      console.error("Error applying update:", error);
    }
  }


  areImagesLoaded(): boolean {
    // Check if promise exists and is resolved
    if (!this.imageLoadPromise) return false;

    // You can track this with a separate boolean
    return this._imagesLoaded || false;
  }


  // Wait for images to load
  async waitForImages(): Promise<void> {
    if (this.imageLoadPromise) {
      await this.imageLoadPromise;
    }
  }

  destroy(): void {
    if (this.imageStorage) {
      this.imageStorage.clearCache();
    }
    if (this.imageDoc) {
      this.imageDoc.destroy();
    }
    // Clear current assets
    this.currentAssets = [];
    if (this.ydoc) {
      this.ydoc.destroy();
    }
    if (this.imageDoc) {
      this.imageDoc.destroy();
    }
  }
  applyAwarenessUpdate(update: Uint8Array | number[], sender: number): void {
    if (sender === this.clientID) {
      return; // Don't apply our own updates
    }

    try {
      const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);

      if (updateArray.length === 0) {
        console.warn("Received empty awareness update");
        return;
      }

      // Apply awareness update using YJS built-in function
      applyAwarenessUpdate(this.awareness, updateArray, 'remote');
    } catch (error) {
      console.error("Error applying awareness update:", error);
    }
  }


  // === Comment System Methods ===

  /**
   * Get the comments service instance
   */
  getCommentsService(): CommentsService {
    return this.commentsService;
  }

  /**
   * Add a reply to an existing comment thread
   */
  addCommentReply(threadId: string, content: string): string | null {
    return this.commentsService.addComment(threadId, content);
  }

  /**
   * Get all comment threads
   */
  getAllCommentThreads(): CommentThread[] {
    return this.commentsService.getAllThreads();
  }

  /**
   * Resolve or unresolve a comment thread
   */
  resolveCommentThread(threadId: string, resolved: boolean): boolean {
    try {
      // 1. Update the thread resolved status
      const result = this.commentsService.resolveThread(threadId, resolved);

      if (result) {
        // 2. Update the visual marks in the editor
        if (this.editorView) {
          const { state, dispatch } = this.editorView;
          let tr = state.tr;
          let marksUpdated = false;

          // Iterate through the document to find and update comment marks with this threadId
          state.doc.descendants((node, pos) => {
            if (node.isText) {
              node.marks.forEach((mark) => {
                if (
                  mark.type.name === "comment" &&
                  mark.attrs.threadId === threadId
                ) {
                  // Remove the old mark and add a new one with updated resolved status
                  tr = tr.removeMark(pos, pos + node.nodeSize, mark);

                  const updatedMark = state.schema.marks.comment.create({
                    ...mark.attrs,
                    resolved,
                  });

                  tr = tr.addMark(pos, pos + node.nodeSize, updatedMark);
                  marksUpdated = true;
                }
              });
            }
          });

          if (marksUpdated) {
            dispatch(tr);
          }
        } else {
          console.warn('No editor view available for updating comment mark resolved status');
        }
      }

      return result;
    } catch (error) {
      console.error('Error resolving comment thread and updating marks:', error);
      return false;
    }
  }

  /**
* Create a comment thread and apply the visual mark to the editor
* This combines comment creation with editor mark application
*/
  createCommentAndApplyMark(position: CommentPosition, content: string): string {
    try {
      console.log('[COMMENTS] Creating comment with position:', position);
      console.log('[COMMENTS] Comment content:', content);
      console.log('[COMMENTS] Editor view available:', !!this.editorView);
      console.log('[COMMENTS] Comments service available:', !!this.commentsService);

      // 1. Create the comment thread directly via comments service
      const threadId = this.commentsService.createThread(position, content);
      console.log('[COMMENTS] Created thread with ID:', threadId);

      // 2. Apply the visual mark to the editor
      if (this.editorView) {
        const { state, dispatch } = this.editorView;
        console.log('[COMMENTS] Current editor state doc size:', state.doc.content.size);
        console.log('[COMMENTS] Selection position:', { from: position.from, to: position.to });

        // Check if position is valid
        if (position.from < 0 || position.to > state.doc.content.size || position.from > position.to) {
          console.error('[COMMENTS] Invalid position for comment:', position);
          return threadId;
        }

        const commentMark = state.schema.marks.comment.create({
          threadId,
          commentIds: [threadId],
          resolved: false,
          author: null,
        });
        console.log('[COMMENTS] Created comment mark:', commentMark);

        const tr = state.tr.addMark(
          position.from,
          position.to,
          commentMark,
        );
        console.log('[COMMENTS] Created transaction with mark');

        dispatch(tr);
        console.log('[COMMENTS] Dispatched transaction');

        // Verify the mark was applied
        setTimeout(() => {
          const newState = this.editorView!.state;
          let foundMark = false;
          newState.doc.nodesBetween(position.from, position.to, (node, pos) => {
            if (node.isText) {
              node.marks.forEach(mark => {
                if (mark.type.name === 'comment' && mark.attrs.threadId === threadId) {
                  foundMark = true;
                  console.log('[COMMENTS] Verified comment mark applied:', mark.attrs);
                }
              });
            }
          });
          if (!foundMark) {
            console.error('[COMMENTS] Comment mark was not found after applying!');
          }
        }, 100);

      } else {
        console.warn('[COMMENTS] No editor view available for applying comment mark');
      }

      return threadId;
    } catch (error) {
      console.error('[COMMENTS] Error creating comment and applying mark:', error);
      throw error;
    }
  }
  /**
 * Remove comment marks from the editor for a specific thread
 */
  removeCommentMark(threadId: string): void {
    if (!this.editorView) {
      console.error("No editor view available");
      return;
    }

    try {
      const { state, dispatch } = this.editorView;
      let tr = state.tr;
      let marksRemoved = false;

      // Iterate through the document to find and remove comment marks with this threadId
      state.doc.descendants((node, pos) => {
        if (node.isText) {
          node.marks.forEach((mark) => {
            if (
              mark.type.name === "comment" &&
              mark.attrs.threadId === threadId
            ) {
              // Remove this specific comment mark
              tr = tr.removeMark(pos, pos + node.nodeSize, mark);
              marksRemoved = true;
            }
          });
        }
      });

      if (marksRemoved) {
        dispatch(tr);
      }

      this.commentsService.deleteThread(threadId);
    } catch (error) {
      console.error("Error removing comment mark:", error);
    }
  }
}

export const notesInstance = new Notes();
