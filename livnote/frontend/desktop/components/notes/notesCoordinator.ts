import { EditorView } from "prosemirror-view";
import { ySyncPlugin, yCursorPlugin, yUndoPlugin, initProseMirrorDoc } from "y-prosemirror";
import { keymap } from "prosemirror-keymap";
import { wrapInList, splitListItem, liftListItem, sinkListItem } from "prosemirror-schema-list";
import { exitCode } from "prosemirror-commands";
import type { Plugin } from "prosemirror-state";

import * as Y from "yjs";
import { YjsManager } from "./collaboration/yjsManager";
import { EditorManager } from "./editor/editorManager";
import { createEditorSchema } from "./schema/editorSchema";
import { CommentsStore } from "./commentsStore";
import { ImageStorageService } from "./imageStorage";

// Import existing plugins
import { slashCommandPlugin } from "./slashCommandPlugin";
import { fixedMenuPlugin } from "./fixedMenuPlugin";
import { floatingMenuPlugin } from "./floatingMenuPlugin";
import { pasteHandlerPlugin } from "./pasteHandlerPlugin";
import { markdownShortcutsPlugin } from "./markdownShortcutsPlugin";
import { imageNodeViewPlugin } from "./imageNodeViewPlugin";
import { dataState } from "../../state";

import type {
  NoteContent,
  UserInfo,
  ImageAsset,
  CommentPosition
} from "../../types/notes.types";

export interface NotesCoordinatorConfig {
  userInfo: UserInfo
  onCollaborationUpdate?: (update: Uint8Array, docType: 'main' | 'images') => void;
  onAwarenessUpdate?: (changes: any) => void;
  onTitleReady?: (title: string) => void; // Add this
}

/**
 * Coordinates between YJS, ProseMirror, and services
 * Simplified interface for the application
 */
export class NotesCoordinator {
  private yjsManager: YjsManager;
  private editorManager: EditorManager;
  private commentsStore: CommentsStore;
  private imageStorage: ImageStorageService | null = null;
  private schema = createEditorSchema();
  private currentAssets: ImageAsset[] = [];
  private userInfo: UserInfo;
  private isLoadingNote: boolean = false;
  constructor(private config: NotesCoordinatorConfig) {
    this.userInfo = config.userInfo; // Initialize userInfo from config
    this.isLoadingNote = true;
    // Initialize YJS manager
    this.yjsManager = new YjsManager({
      clientId: this.userInfo.id, // Use the userInfo property
      onUpdate: (update, origin, docType) => {
        if (config.onCollaborationUpdate) {
          config.onCollaborationUpdate(update, docType);
        }
      },
      onAwarenessChange: (changes, origin) => {
        if (origin === 'local' && config.onAwarenessUpdate) {
          config.onAwarenessUpdate(changes);
        }
      },
      onAfterAllTransactions: () => {
        // Only trigger title update if we're loading a note
        if (this.isLoadingNote && config.onTitleReady) {
          const title = this.yjsManager.getMetadata("title") || "Untitled Note";
          console.log("afterAllTransactions: title from YJS:", title);
          config.onTitleReady(title);
          this.isLoadingNote = false; // Reset flag
        }
      }
    });

    const docs = this.yjsManager.initialize();

    // Initialize services once
    this.commentsStore = new CommentsStore();
    this.commentsStore.setCurrentUser(this.userInfo);
    this.imageStorage = new ImageStorageService(docs.imagesMap, this.userInfo.id);
    this.yjsManager.setUserInfo(this.userInfo);
    // Initialize editor manager
    this.editorManager = new EditorManager({
      schema: this.schema,
      onTransaction: (tr, newState) => {
        // Handle transaction updates if needed
      }
    });
  }
  /**
   * Load existing note
   */
  /**
   * Load existing note
   */
  async loadNote(noteContent: NoteContent): Promise<void> {
    console.log("🔄 Loading note, reinitializing YJS documents");
    this.isLoadingNote = true;
    // Reinitialize YJS to ensure clean state
    const docs = this.yjsManager.initialize(); // This destroys old docs and creates new ones

    // Re-setup services with new documents
    this.commentsStore.setCommentsMap(docs.commentsMap);
    this.imageStorage = new ImageStorageService(docs.imagesMap, this.userInfo.id);

    console.log("📦 Setting up assets");
    this.currentAssets = noteContent.assets || [];
    this.imageStorage.setAssetsArray(this.currentAssets);

    console.log("📊 YJS state length:", noteContent.yjs_state?.length || 0);
    console.log("🖼️ Image state length:", noteContent.image_state?.length || 0);

    // Apply YJS state if available
    if (noteContent.yjs_state && noteContent.yjs_state.length > 0) {
      console.log("🔄 Applying main YJS state");
      this.yjsManager.applyUpdate(noteContent.yjs_state, 'main', 'loading');
    }

    // Apply image metadata state
    if (noteContent.image_state && noteContent.image_state.length > 0) {
      console.log("🔄 Applying image YJS state");
      this.yjsManager.applyUpdate(noteContent.image_state, 'images', 'loading');
    }

    // Wait a tick for YJS to process
    await new Promise(resolve => setTimeout(resolve, 0));

    console.log("💬 Comments in map after applying updates:", docs.commentsMap.size);


    // Create editor plugins with fresh documents
    const plugins = this.createEditorPlugins(docs);

    // Initialize ProseMirror doc from YJS
    const prosemirrorDoc = initProseMirrorDoc(docs.type, this.schema);

    if (prosemirrorDoc.doc.childCount === 0 && noteContent.editor_state?.doc) {
      const fallbackDoc = this.schema.nodeFromJSON(noteContent.editor_state.doc);
      this.editorManager.initializeState(fallbackDoc, plugins);
    } else {
      this.editorManager.initializeState(prosemirrorDoc.doc, plugins);
    }
  }
  /**
   * Create editor view in container
   */
  createEditorView(container: HTMLElement): EditorView {
    const view = this.editorManager.createView(container);
    // Apply any pending YJS state after view is created
    const docs = this.yjsManager.getDocuments();
    if (docs && view) {
      console.log('🔄 Triggering sync transaction');
      // Trigger a transaction to sync the view
      const tr = view.state.tr;
      view.dispatch(tr);
    }

    return view;
  }
  /**
   * Create all editor plugins
   */
  private createEditorPlugins(docs: any): Plugin[] {
    const listKeymap = keymap({
      Enter: splitListItem(this.schema.nodes.list_item),
      Tab: sinkListItem(this.schema.nodes.list_item),
      "Shift-Tab": liftListItem(this.schema.nodes.list_item),
      "Ctrl-Shift-8": wrapInList(this.schema.nodes.bullet_list),
      "Ctrl-Shift-9": wrapInList(this.schema.nodes.ordered_list),
    });

    const codeBlockKeymap = keymap({
      "Shift-Enter": exitCode,
      Enter: (state, dispatch) => {
        if (state.selection.$from.parent.type === this.schema.nodes.code_block) {
          if (dispatch) {
            dispatch(state.tr.insertText("\n"));
          }
          return true;
        }
        return false;
      },
    });

    const hardBreakKeymap = keymap({
      "Shift-Enter": (state, dispatch) => {
        const { $from } = state.selection;
        if ($from.parent.type.name === "code_block") return false;

        if (dispatch) {
          const hardBreak = state.schema.nodes.hard_break.create();
          dispatch(state.tr.replaceSelectionWith(hardBreak).scrollIntoView());
        }
        return true;
      }
    });

    return [
      ySyncPlugin(docs.type),
      yCursorPlugin(docs.awareness, {
        cursorBuilder: this.createCustomCursor.bind(this),
      }),
      yUndoPlugin(),
      listKeymap,
      codeBlockKeymap,
      hardBreakKeymap,
      pasteHandlerPlugin(this.imageStorage!),
      slashCommandPlugin(this.schema),
      fixedMenuPlugin(this.schema),
      floatingMenuPlugin(this.schema),
      markdownShortcutsPlugin(this.schema),
      imageNodeViewPlugin(this.imageStorage!),
    ];
  }

  /**
   * Create custom cursor for collaboration
   */
  private createCustomCursor(user: UserInfo): HTMLElement {
    const cursor = document.createElement('span');

    if (user.id === this.userInfo.id) {
      return cursor
    }
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

  /**
   * Save current note
   */
  saveNote(title?: string): NoteContent {
    if (title !== undefined) {
      this.yjsManager.setMetadata("title", title);
    }

    return this.createNoteContent();
  }

  getTitle(): string {
    return this.yjsManager.getMetadata("title");
  }

  /**
   * Create note content from current state
   */
  private createNoteContent(): NoteContent {
    const docs = this.yjsManager.getDocuments();
    if (!docs) {
      throw new Error("YJS documents not initialized");
    }

    const editorState = this.editorManager.getState();
    if (!editorState) {
      throw new Error("Editor state not initialized");
    }

    return {
      content: docs.type.toJSON(),
      yjs_state: Array.from(this.yjsManager.getStateAsUpdate('main')),
      image_state: Array.from(this.yjsManager.getStateAsUpdate('images')),
      assets: this.currentAssets,
      editor_state: editorState.toJSON(),
      client_id: this.userInfo.id.toString(),
      last_modified: Date.now(),
      title: this.yjsManager.getMetadata("title") || "Untitled Note",
    };
  }
  /**
   * Apply remote update
   */
  applyRemoteUpdate(update: Uint8Array | number[], sender: number, docType: 'main' | 'images' = 'main'): void {
    if (sender === this.userInfo.id) return;

    this.yjsManager.applyUpdate(update, docType, 'sync');
  }



  /**
   * Get current title
   */
  getCurrentTitle(): string {
    return this.yjsManager.getMetadata("title") || "Untitled Note";
  }


  /**
   * Create comment and apply mark
   */
  createComment(position: CommentPosition, content: string): string {
    if (!this.commentsStore) {
      throw new Error("Comments service not initialized");
    }

    const threadId = this.commentsStore.createThread(position, content);


    // Apply mark to editor
    const view = this.editorManager.getView();
    if (view) {
      const { state, dispatch } = view;
      const commentMark = this.schema.marks.comment.create({
        threadId,
        commentIds: [threadId],
        resolved: false,
        author: null,
      });

      const tr = state.tr.addMark(position.from, position.to, commentMark);
      dispatch(tr);
    }

    return threadId;
  }

  /**
   * Clean up resources
   */
  destroy(): void {
    this.editorManager.destroy();
    this.yjsManager.destroy();
    this.imageStorage?.clearCache();
    this.currentAssets = [];
    this.commentsStore.destroy();
  }

  /**
     * Get the comments store for UI components
     */
  getCommentsStore(): CommentsStore {
    return this.commentsStore;
  }
  getImageStorage(): ImageStorageService | null {
    return this.imageStorage;
  }

  getEditorView(): EditorView | null {
    return this.editorManager.getView();
  }
}
