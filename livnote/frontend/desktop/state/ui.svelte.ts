
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
  isNavigationPanelManuallyToggled = $state(false);
  isNoteFetching = $state<boolean>(false);
  isEditorLoading = $state<boolean>(false);
  readonly MIN_EDITOR_WIDTH = 900; // Minimum editor width in pixels

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

  resetNavigationPanelManualToggle() {
    this.isNavigationPanelManuallyToggled = false;
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

  clearAllLoadingStates() {
    this.isNoteFetching = false;
    this.isEditorLoading = false;
  }

  get loadingPhase(): 'idle' | 'fetching' | 'editor' | 'ready' {
    if (this.isNoteFetching) return 'fetching';
    if (this.isEditorLoading) return 'editor';
    return 'ready';
  }

}

export const uiState = new UIState();
