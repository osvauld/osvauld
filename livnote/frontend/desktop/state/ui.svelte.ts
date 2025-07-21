// state/ui.state.ts

// Define UI-specific interfaces
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



// Modal registry type to make modal management more structured
type ModalKey = 'showConnector' | 'showSyncQr';

// UI State class
class UIState {
  // Layout state
  noteViewLayout = $state(false);
  profileViewLayout = $state(false);
  showWelcome = $state(true);
  showSyncQr = $state(false);
  vaultManagerActive = $state(false);
  noteSaved = $state(false);

  // Navigation panel state
  showNavigationPanel = $state(true);
  isNavigationPanelManuallyToggled = $state(false);
  isNoteFetching = $state<boolean>(false);
  isEditorLoading = $state<boolean>(false);
  readonly MIN_EDITOR_WIDTH = 1300; // Minimum editor width in pixels

  // Modal states
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

  // Simple boolean modals
  showConnector = $state(false);

  // Modal registry for type-safe modal management
  get modalRegistry(): Record<ModalKey, boolean> {
    return {
      showConnector: this.showConnector,
      showSyncQr: this.showSyncQr
    };
  }

  // Toast management
  showToast(message: string, success: boolean = true) {
    this.toastMessage.show = true;
    this.toastMessage.message = message;
    this.toastMessage.success = success;

    // Auto-hide toast after 3 seconds
    setTimeout(() => {
      this.toastMessage.show = false;
    }, 3000);
  }

  // Vault manager
  toggleVaultManager() {
    this.vaultManagerActive = !this.vaultManagerActive;
  }


  // Delete confirmation modal management
  showDeleteConfirmation(item: string) {
    this.deleteConfirmationModal.item = item;
    this.deleteConfirmationModal.show = true;
  }

  hideDeleteConfirmation() {
    this.deleteConfirmationModal.item = "";
    this.deleteConfirmationModal.show = false;
  }

  // Password prompt modal management
  showPasswordPrompt(isChangePassword: boolean = false) {
    this.passwordPromptModal.isChangePassword = isChangePassword;
    this.passwordPromptModal.show = true;
  }

  hidePasswordPrompt() {
    this.passwordPromptModal.isChangePassword = false;
    this.passwordPromptModal.show = false;
  }

  // Generic modal toggle function
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

  // Helper to close all modals (useful for global escape key handling)
  closeAllModals() {
    // Close all simple boolean modals
    Object.keys(this.modalRegistry).forEach(key => {
      this.toggleModal(key as ModalKey, false);
    });

    // Close complex modals
    this.hideDeleteConfirmation();
    this.hidePasswordPrompt();
  }

  // Toggle layout view
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

  // Welcome screen management
  setWelcomeScreen(show: boolean) {
    this.showWelcome = show;
  }

  // Toggle navigation panel
  toggleNavigationPanel(show?: boolean) {
    if (show !== undefined) {
      this.showNavigationPanel = show;
    } else {
      this.showNavigationPanel = !this.showNavigationPanel;
    }

    // Mark panel as manually toggled
    this.isNavigationPanelManuallyToggled = true;
  }

  // Reset manual toggle flag
  resetNavigationPanelManualToggle() {
    this.isNavigationPanelManuallyToggled = false;
  }
  get isNoteLoading(): boolean {
    return this.isNoteFetching || this.isEditorLoading;
  }

  // Methods to manage loading states
  setNoteFetching(fetching: boolean) {
    this.isNoteFetching = fetching;
  }

  setEditorLoading(loading: boolean) {
    this.isEditorLoading = loading;
  }

  // Clear all loading states
  clearAllLoadingStates() {
    this.isNoteFetching = false;
    this.isEditorLoading = false;
  }

  // Get current loading phase for skeleton display
  get loadingPhase(): 'idle' | 'fetching' | 'editor' | 'ready' {
    if (this.isNoteFetching) return 'fetching';
    if (this.isEditorLoading) return 'editor';
    return 'ready';
  }

}

// Create the singleton UI state
export const uiState = new UIState();
