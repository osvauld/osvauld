
interface Toast {
  show: boolean;
  message: string;
  success: boolean;
}

interface DeleteConfirmation {
  item: string;
  show: boolean;
}

interface PasswordPrompt {
  isChangePassword: boolean;
  show: boolean;
}

interface ConnectUserModal {
  show: boolean;
}

type ModalKey = 'showConnector' | 'showSyncQr';

class UIState {
  noteViewLayout = $state(false);
  profileViewLayout = $state(false);
  showWelcome = $state(true);
  showSyncQr = $state(false);
  vaultManagerActive = $state(false);
  noteSaved = $state(false);

  showNavigationPanel = $state(true);
  showNoteRightPanel = $state(true);
  isNavigationPanelManuallyToggled = $state(false);
  isNoteRightPanelManuallyToggled = $state(false);
  isNoteFetching = $state<boolean>(false);
  isEditorLoading = $state<boolean>(false);
  isZenMode = $state<boolean>(false);
  readonly MIN_EDITOR_WIDTH = 900; // Minimum editor width in pixels
  readonly NOTE_RIGHT_PANEL_WIDTH = 360; // Note right panel width in pixels
  readonly NAV_PANEL_WIDTH = 360; // Navigation panel width in pixels
  
  // Track expanded folders in navigation tree
  expandedFolders = $state<Set<string>>(new Set());


  toastMessage = $state<Toast>({
    show: false, message: "", success: true
  });

  deleteConfirmationModal = $state<DeleteConfirmation>({
    item: "", show: false
  });

  passwordPromptModal = $state<PasswordPrompt>({
    isChangePassword: false,
    show: false,
  });

  connectUserModal = $state<ConnectUserModal>({
    show: false,
  });

  showConnector = $state(false);

  get modalRegistry(): Record<ModalKey, boolean> {
    return {
      showConnector: this.showConnector,
      showSyncQr: this.showSyncQr
    };
  }

  showToast(message: string, success: boolean = true) {
    this.toastMessage.show = true;
    this.toastMessage.message = message;
    this.toastMessage.success = success;

    setTimeout(() => {
      this.toastMessage.show = false;
    }, 3000);
  }

  toggleVaultManager() {
    this.vaultManagerActive = !this.vaultManagerActive;
  }


  showDeleteConfirmation(item: string) {
    this.deleteConfirmationModal.item = item;
    this.deleteConfirmationModal.show = true;
  }

  hideDeleteConfirmation() {
    this.deleteConfirmationModal.item = "";
    this.deleteConfirmationModal.show = false;
  }

  showPasswordPrompt(isChangePassword: boolean = false) {
    this.passwordPromptModal.isChangePassword = isChangePassword;
    this.passwordPromptModal.show = true;
  }

  hidePasswordPrompt() {
    this.passwordPromptModal.isChangePassword = false;
    this.passwordPromptModal.show = false;
  }

  showConnectUserModal() {
    this.connectUserModal.show = true;
  }

  hideConnectUserModal() {
    this.connectUserModal.show = false;
  }

  toggleModal(modalKey: ModalKey, value?: boolean) {
    switch (modalKey) {
      case 'showConnector':
        this.showConnector = value !== undefined ? value : !this.showConnector;
        break;
      case 'showSyncQr':
        this.showSyncQr = value !== undefined ? value : !this.showSyncQr;
        break;
    }
  }

  closeAllModals() {
    Object.keys(this.modalRegistry).forEach(key => {
      this.toggleModal(key as ModalKey, false);
    });

    this.hideDeleteConfirmation();
    this.hidePasswordPrompt();
  }

  toggleNoteViewLayout(show?: boolean) {
    if (show !== undefined) {
      this.noteViewLayout = show;
    } else {
      this.noteViewLayout = !this.noteViewLayout;
    }
  }

  toggleProfileViewLayout(show?: boolean) {
    if (show !== undefined) {
      this.profileViewLayout = show;
    } else {
      this.profileViewLayout = !this.profileViewLayout;
    }
  }

  setWelcomeScreen(show: boolean) {
    this.showWelcome = show;
  }

  toggleNavigationPanel(show?: boolean) {
    if (show !== undefined) {
      this.showNavigationPanel = show;
    } else {
      this.showNavigationPanel = !this.showNavigationPanel;
    }
    this.isNavigationPanelManuallyToggled = true;
  }

  toggleNoteRightPanel(show?: boolean) {
    if (show !== undefined) {
      this.showNoteRightPanel = show;
    } else {
      this.showNoteRightPanel = !this.showNoteRightPanel;
    }
    this.isNoteRightPanelManuallyToggled = true;
  }

  resetNavigationPanelManualToggle() {
    this.isNavigationPanelManuallyToggled = false;
  }

  resetNoteRightPanelManualToggle() {
    this.isNoteRightPanelManuallyToggled = false;
  }

  /**
   * Calculate the minimum viewport width needed to show both panels and editor
   */
  get minViewportWidthForBothPanels(): number {
    return this.NAV_PANEL_WIDTH + this.MIN_EDITOR_WIDTH + this.NOTE_RIGHT_PANEL_WIDTH;
  }

  /**
   * Calculate the minimum viewport width needed to show nav panel and editor
   */
  get minViewportWidthForNavPanel(): number {
    return this.NAV_PANEL_WIDTH + this.MIN_EDITOR_WIDTH;
  }

  /**
   * Calculate the minimum viewport width needed to show editor and right panel
   */
  get minViewportWidthForRightPanel(): number {
    return this.MIN_EDITOR_WIDTH + this.NOTE_RIGHT_PANEL_WIDTH;
  }
  get isNoteLoading(): boolean {
    return this.isNoteFetching || this.isEditorLoading;
  }

  setNoteFetching(fetching: boolean) {
    this.isNoteFetching = fetching;
  }

  setEditorLoading(loading: boolean) {
    this.isEditorLoading = loading;
  }

  setNoteSaved(saved: boolean) {
    this.noteSaved = saved;
  }

  toggleZenMode() {
    this.isZenMode = !this.isZenMode;
  }

  clearAllLoadingStates() {
    this.isNoteFetching = false;
    this.isEditorLoading = false;
  }

  get loadingPhase(): 'idle' | 'fetching' | 'editor' | 'ready' {
    if (this.isNoteFetching) return 'fetching';
    if (this.isEditorLoading) return 'editor';
    return 'ready';
  }

  // Folder expansion management
  toggleFolderExpansion(folderId: string) {
    if (folderId === "all") return; // Don't allow All Notes folder to be toggled
    
    if (this.expandedFolders.has(folderId)) {
      this.expandedFolders.delete(folderId);
    } else {
      this.expandedFolders.add(folderId);
    }
    // Trigger reactivity
    this.expandedFolders = new Set(this.expandedFolders);
  }

  expandFolder(folderId: string) {
    if (folderId === "all") return;
    
    if (!this.expandedFolders.has(folderId)) {
      this.expandedFolders.add(folderId);
      this.expandedFolders = new Set(this.expandedFolders);
    }
  }

  isFolderExpanded(folderId: string): boolean {
    return this.expandedFolders.has(folderId);
  }

}

export const uiState = new UIState();
