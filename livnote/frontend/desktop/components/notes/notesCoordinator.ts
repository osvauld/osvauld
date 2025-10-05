import { EditorView } from "prosemirror-view";
import { ySyncPlugin, yCursorPlugin, yUndoPlugin, initProseMirrorDoc } from "y-prosemirror";
import { keymap } from "prosemirror-keymap";
import { wrapInList, splitListItem, liftListItem, sinkListItem } from "prosemirror-schema-list";
import { exitCode } from "prosemirror-commands";
import type { Plugin } from "prosemirror-state";
import { YjsManager } from "./yjsManager";
import { EditorManager } from "./editorManager";
import { createEditorSchema } from "./schema/editorSchema";
import { CommentsStore } from "./commentsStore";
import { ImageStorageService } from "./imageStorage";
import { getTablePlugins } from "./setup/tablePlugin";
import { createMathPlugins, mathBackspaceCmd } from "./mathPlugin";
import { chainCommands, deleteSelection, joinBackward, selectNodeBackward } from "prosemirror-commands";
// Import existing plugins
import { slashCommandPlugin } from "./slashCommandPlugin";
import { fixedMenuPlugin } from "./fixedMenuPlugin";
import { floatingMenuPlugin } from "./floatingMenuPlugin";
import { pasteHandlerPlugin } from "./pasteHandlerPlugin";
import { markdownShortcutsPlugin } from "./markdownShortcutsPlugin";
import { imageNodeViewPlugin } from "./imageNodeViewPlugin";
import { dataState } from "../../state";
import { SearchManager } from "./SearchManager";
import type {
  NoteContent,
  UserInfo,
  ImageAsset,
  CommentPosition
} from "../../types/notes.types";

export interface NotesCoordinatorConfig {
  userInfo: UserInfo
  onCollaborationUpdate?: (update: Uint8Array, docType: 'main' | 'images' | 'comments') => void;
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
  private userInfo: UserInfo;
  private searchManager: SearchManager;
  private _imageStoreHandler: ((event: CustomEvent) => void) | null = null;
  constructor(private config: NotesCoordinatorConfig) {
    this.userInfo = config.userInfo;
    this.searchManager = new SearchManager();
    this.yjsManager = new YjsManager({
      clientId: this.userInfo.id,
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
    this.commentsStore = new CommentsStore();
    this.commentsStore.setCurrentUser(this.userInfo);
    this.imageStorage = new ImageStorageService(this.userInfo.id);
    this.yjsManager.setUserInfo(this.userInfo);
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
  async loadNote(noteContent: NoteContent): Promise<void> {
    const docs = this.yjsManager.initialize();
    this.commentsStore.setCommentsMap(docs.threads, docs.replies, docs.replyContents);
    this.imageStorage?.setImageMap(docs.imagesMap);
    
    docs.mainDoc.once('afterAllTransactions', () => {
      this.handleMainDocReady(docs, noteContent);
    });
    
    this.yjsManager.applyUpdate(noteContent.main_doc, 'main', 'loading');
  }

  private handleMainDocReady(docs: any, noteContent: NoteContent): void {
    this.yjsManager.setUserInfo(this.userInfo);
    const title = this.yjsManager.getMetadata("title") || "Untitled";
    dataState.currentNoteTitle = title;
    const plugins = this.createEditorPlugins(docs);
    const prosemirrorDoc = initProseMirrorDoc(docs.type, this.schema);
    this.editorManager.initializeState(prosemirrorDoc.doc, plugins);
    
    document.dispatchEvent(new CustomEvent('editor-view-ready', {
      detail: {
        getEditorManager: () => this.editorManager,
        getSearchManager: () => this.searchManager
      }
    }));
    document.dispatchEvent(new CustomEvent('comments-store-ready', {
      detail: { commentsStore: this.commentsStore }
    }));

    // Sync collaborators after note is loaded
    this.yjsManager.syncCollaboratorsToDataState();

    // Defer image loading to next event loop - makes editor appear instantly
    setTimeout(() => {
      this.deferImageLoading(noteContent);
    }, 0);
  }
  private async deferImageLoading(noteContent: NoteContent): Promise<void> {
    if (noteContent.image_state && noteContent.image_state.length > 0) {
      this.yjsManager.applyUpdate(noteContent.image_state, 'images', 'loading');
    }
    this.imageStorage?.initializeCacheFromYjs();
    this.deferCommentLoading(noteContent);
    document.dispatchEvent(new CustomEvent('assets-loaded'));
  }

  private async deferCommentLoading(noteContent: NoteContent): Promise<void> {
    if (noteContent.comment_state && noteContent.comment_state.length > 0) {
      this.yjsManager.applyUpdate(noteContent.comment_state, 'comments', 'loading');
    }
  }
  /**
   * Create editor view in container
   */
  createEditorView(container: HTMLElement): EditorView {
    const view = this.editorManager.createView(container);
    const docs = this.yjsManager.getDocuments();
    if (docs && view) {
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
    const mathKeymap = keymap({
      "Backspace": chainCommands(
        deleteSelection,
        mathBackspaceCmd,
        joinBackward,
        selectNodeBackward
      )
    });

    return [
      ySyncPlugin(docs.type),
      yCursorPlugin(docs.awareness, {
        awarenessStateFilter: (state: any, clientId: number) => {
          return clientId !== docs.mainDoc.clientID;
        },

        cursorBuilder: this.createCustomCursor.bind(this),
      }),
      yUndoPlugin(),
      this.searchManager.getPlugin(),
      listKeymap,
      codeBlockKeymap,
      hardBreakKeymap,
      mathKeymap,
      ...createMathPlugins(this.schema),
      pasteHandlerPlugin(this.imageStorage!),
      slashCommandPlugin(this.schema),
      fixedMenuPlugin(this.schema),
      floatingMenuPlugin(this.schema, this.searchManager),
      markdownShortcutsPlugin(this.schema),
      imageNodeViewPlugin(this.imageStorage!),
      ...getTablePlugins(this.schema),
    ];
  }
  getSearchManager(): SearchManager {
    return this.searchManager;
  }
  private setupImageStoreListener(): void {
    const handleStoreImageRequest = async (event: CustomEvent) => {
      const { dataUrl, mimeType, filename, callback } = event.detail;

      if (!this.imageStorage) {
        console.error('Image storage not initialized');
        return;
      }

      try {
        const imageId = await this.imageStorage.storeImage(dataUrl, mimeType, filename);
        const metadata = this.imageStorage.getImageMetadata(imageId);
        if (callback && typeof callback === 'function') {
          callback(imageId, metadata);
        }

      } catch (error) {
        console.error('Error storing image:', error);
      }
    };
    document.addEventListener('store-image-request', handleStoreImageRequest as EventListener);
    this._imageStoreHandler = handleStoreImageRequest;
  }

  /**
   * Create custom cursor for collaboration
   */
  private createCustomCursor(user: UserInfo): HTMLElement {
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
      main_doc: Array.from(this.yjsManager.getStateAsUpdate('main')),
      image_state: Array.from(this.yjsManager.getStateAsUpdate('images')),
      comment_state: Array.from(this.yjsManager.getStateAsUpdate('comments')),
      client_id: this.userInfo.id.toString(),
      last_modified: Date.now(),
      title: this.yjsManager.getMetadata("title") || "Untitled",
    };
  }

  /**
   * Apply remote update
   */
  applyRemoteUpdate(update: Uint8Array | number[], sender: number, docType: 'main' | 'images' = 'main'): void {
    if (sender === this.userInfo.id) return;

    this.yjsManager.applyUpdate(update, docType, 'sync');
  }

  applyAwarenessUpdate(update: Uint8Array, sender: number) {
    this.yjsManager.applyAwarenessUpdate(update, sender);
  }

  /**
   * Sync collaborators from awareness state
   */
  syncCollaborators(): void {
    this.yjsManager.syncCollaboratorsToDataState();
  }



  /**
   * Get current title
   */
  getCurrentTitle(): string {
    return this.yjsManager.getMetadata("title") || "Untitled";
  }


  /**
   * Create comment and apply mark
   */
  createComment(position: CommentPosition, content: string): string {
    if (!this.commentsStore) {
      throw new Error("Comments service not initialized");
    }

    const threadId = this.commentsStore.createThread(position, content);
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
    this.imageStorage?.destroy();
    this.commentsStore.destroy();
    if (this._imageStoreHandler) {
      document.removeEventListener('store-image-request', this._imageStoreHandler as EventListener);
    }

    // Clear collaborators when destroying
    dataState.updateCollaborators([]);
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


  async getStateVectors(): Promise<Record<string, any>> {
    const result: Record<string, { updates: number[]; state_vector: number[] }> = {};
    const mainStateVector = this.yjsManager.getStateVector('main');
    const imageStateVector = this.yjsManager.getStateVector('images');
    const commentStateVector = this.yjsManager.getStateVector('comments');
    result['main_doc'] = {
      updates: [],
      state_vector: Array.from(mainStateVector)
    };
    result['image_state'] = {
      updates: [],
      state_vector: Array.from(imageStateVector)
    };
    result['comment_state'] = {
      updates: [],
      state_vector: Array.from(commentStateVector)
    };
    return result;
  }
}
