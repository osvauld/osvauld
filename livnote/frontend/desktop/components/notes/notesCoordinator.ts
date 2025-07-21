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
  private imageLoadStartTime: number = 0;
  private imageLoadEndTime: number = 0;
  private mainDocLoadTime: number = 0;
  private _imageStoreHandler: ((event: CustomEvent) => void) | null = null;
  constructor(private config: NotesCoordinatorConfig) {
    this.userInfo = config.userInfo; // Initialize userInfo from config
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
    this.setupImageStoreListener();
  }
  /**
   * Load existing note
   */
  // Update the loadNote method
  async loadNote(noteContent: NoteContent): Promise<void> {
    const totalStartTime = performance.now();
    console.log("🔄 Starting note loading");

    // Reinitialize YJS to ensure clean state
    const docs = this.yjsManager.initialize();

    // Re-setup services with new documents
    this.commentsStore.setCommentsMap(docs.commentsMap);
    this.imageStorage = new ImageStorageService(docs.imagesMap, this.userInfo.id);

    // Start performance tracking
    this.imageStorage.startLoadTracking();

    // Track main doc load time
    const mainDocStart = performance.now();

    // Set up single event handler for when main doc is ready
    docs.mainDoc.once('afterAllTransactions', () => {
      this.mainDocLoadTime = performance.now() - mainDocStart;
      console.log(`📄 Main doc loaded in ${this.mainDocLoadTime}ms`);
      this.handleMainDocReady(docs, noteContent);
    });

    // Apply main YJS state
    if (noteContent.yjs_state && noteContent.yjs_state.length > 0) {
      console.log("🔄 Applying main YJS state");
      this.yjsManager.applyUpdate(noteContent.yjs_state, 'main', 'loading');
    } else {
      // No YJS state, trigger manually
      setTimeout(() => {
        this.mainDocLoadTime = performance.now() - mainDocStart;
        this.handleMainDocReady(docs, noteContent);
      }, 0);
    }
  }

  private handleMainDocReady(docs: any, noteContent: NoteContent): void {
    console.log("📄 Main doc ready, setting up editor");
    this.yjsManager.setUserInfo(this.userInfo);

    // Set title
    const title = this.yjsManager.getMetadata("title") || "Untitled Note";
    dataState.currentNoteTitle = title;

    // Create editor
    const plugins = this.createEditorPlugins(docs);
    const prosemirrorDoc = initProseMirrorDoc(docs.type, this.schema);

    if (prosemirrorDoc.doc.childCount === 0 && noteContent.editor_state?.doc) {
      const fallbackDoc = this.schema.nodeFromJSON(noteContent.editor_state.doc);
      this.editorManager.initializeState(fallbackDoc, plugins);
    } else {
      this.editorManager.initializeState(prosemirrorDoc.doc, plugins);
    }

    // Emit editor ready
    document.dispatchEvent(new CustomEvent('editor-view-ready', {
      detail: { getEditorManager: () => this.editorManager }
    }));

    // Emit comments ready
    document.dispatchEvent(new CustomEvent('comments-store-ready', {
      detail: { commentsStore: this.commentsStore }
    }));

    // Start deferred image loading
    this.deferImageLoading(noteContent);
  }
  private async deferImageLoading(noteContent: NoteContent): Promise<void> {
    this.imageLoadStartTime = performance.now();
    console.log("🖼️ Starting deferred image loading");

    // Apply image YJS state if available
    if (noteContent.image_state && noteContent.image_state.length > 0) {
      console.log(`🔄 Applying image YJS state (${noteContent.image_state.length} bytes)`);
      this.yjsManager.applyUpdate(noteContent.image_state, 'images', 'loading');
    }

    // Initialize cache from YJS
    this.imageStorage?.initializeCacheFromYjs();

    this.imageLoadEndTime = performance.now();
    const loadTime = this.imageLoadEndTime - this.imageLoadStartTime;

    // Get metrics
    const metrics = this.imageStorage?.getLoadMetrics();

    console.log(`✅ Images loaded in ${loadTime}ms`);
    console.log(`📊 Image metrics:`, metrics);

    // Emit performance data
    document.dispatchEvent(new CustomEvent('performance-metrics', {
      detail: {
        mainDocLoadTime: this.mainDocLoadTime,
        imageLoadTime: loadTime,
        imageMetrics: metrics,
        totalLoadTime: performance.now() - this.imageLoadStartTime
      }
    }));

    // Notify image nodes
    document.dispatchEvent(new CustomEvent('assets-loaded'));
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
  private setupImageStoreListener(): void {
    const handleStoreImageRequest = async (event: CustomEvent) => {
      const { dataUrl, mimeType, filename, callback } = event.detail;

      if (!this.imageStorage) {
        console.error('Image storage not initialized');
        return;
      }

      try {
        // Store the image using the image storage service
        const imageId = await this.imageStorage.storeImage(dataUrl, mimeType, filename);

        // Get metadata for dimensions
        const metadata = this.imageStorage.getImageMetadata(imageId);

        // Call the callback with the image ID and metadata
        if (callback && typeof callback === 'function') {
          callback(imageId, metadata);
        }

        console.log(`[NotesCoordinator] Image stored via menu: ${imageId}`);
      } catch (error) {
        console.error('Error storing image:', error);
      }
    };

    document.addEventListener('store-image-request', handleStoreImageRequest as EventListener);

    // Store the handler for cleanup
    this._imageStoreHandler = handleStoreImageRequest;
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

    // No need for assets array anymore - everything is in YJS
    return {
      content: docs.type.toJSON(),
      yjs_state: Array.from(this.yjsManager.getStateAsUpdate('main')),
      image_state: Array.from(this.yjsManager.getStateAsUpdate('images')),
      assets: [], // Empty array since we're using YJS
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
    if (this._imageStoreHandler) {
      document.removeEventListener('store-image-request', this._imageStoreHandler as EventListener);
    }
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
