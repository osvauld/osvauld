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
interface VaultManager {
  isActive: boolean;
  source: 'nav' | 'content';
}

// Modal registry type to make modal management more structured
type ModalKey =
  'showConnector' | 'showAddUser' | 'showSyncQr';

// Create a root state object for UI elements
const createUIState = () => {
  // UI state
  const state = {
    // Layout state
    noteViewLayout: $state<boolean>(false),
    showWelcome: $state<boolean>(true),
    showSyncQr: $state<boolean>(false),
    vaultManager: $state<VaultManager>({
      isActive: false,
      source: 'content'
    }),

    // Modal states
    toastMessage: $state<Toast>({
      show: false, message: "", success: true
    }),
    deleteConfirmationModal: $state<DeleteConfirmation>({
      item: "", show: false
    }),
    passwordPromptModal: $state<PasswordPrompt>({
      isChangePassword: false,
      show: false,
    }),

    // Simple boolean modals
    showConnector: $state<boolean>(false),
    showAddUser: $state<boolean>(false),
  };

  // ModalRegistry to make modal management more type-safe
  const modalRegistry: Record<ModalKey, boolean> = {
    showConnector: state.showConnector,
    showAddUser: state.showAddUser,
    showSyncQr: state.showSyncQr
  };

  // UI actions
  const actions = {
    // Toast management
    showToast(message: string, success: boolean = true) {
      state.toastMessage = {
        show: true,
        message,
        success
      };

      // Auto-hide toast after 3 seconds
      setTimeout(() => {
        state.toastMessage = {
          ...state.toastMessage,
          show: false
        };
      }, 3000);
    },

    toggleVaultManager(source: 'nav' | 'content') {
      state.vaultManager = {
        isActive: !state.vaultManager.isActive,
        source
      };
    },
    closeVaultManager() {
      state.vaultManager = {
        ...state.vaultManager,
        isActive: false
      };
    },

    // Delete confirmation modal management
    showDeleteConfirmation(item: string) {
      state.deleteConfirmationModal = {
        item,
        show: true
      };
    },

    hideDeleteConfirmation() {
      state.deleteConfirmationModal = {
        item: "",
        show: false
      };
    },

    // Password prompt modal management
    showPasswordPrompt(isChangePassword: boolean = false) {
      state.passwordPromptModal = {
        isChangePassword,
        show: true
      };
    },

    hidePasswordPrompt() {
      state.passwordPromptModal = {
        isChangePassword: false,
        show: false
      };
    },

    // Generic modal toggle function
    toggleModal(modalKey: ModalKey, value?: boolean) {
      // Type assertion to allow dynamic property access
      const stateKey = modalKey as keyof typeof state;

      if (typeof state[stateKey] === 'boolean') {
        if (value !== undefined) {
          // @ts-ignore - We've checked the type above
          state[stateKey] = value;
        } else {
          // @ts-ignore - We've checked the type above
          state[stateKey] = !state[stateKey];
        }
      }
    },

    // Helper to close all modals (useful for global escape key handling)
    closeAllModals() {
      // Close all simple boolean modals
      Object.keys(modalRegistry).forEach(key => {
        actions.toggleModal(key as ModalKey, false);
      });

      // Close complex modals
      actions.hideDeleteConfirmation();
      actions.hidePasswordPrompt();
    },

    // Toggle layout view
    toggleNoteViewLayout(show?: boolean) {
      if (show !== undefined) {
        state.noteViewLayout = show;
      } else {
        state.noteViewLayout = !state.noteViewLayout;
      }
    },

    // Welcome screen management
    setWelcomeScreen(show: boolean) {
      state.showWelcome = show;
    }
  };

  return {
    ...state,
    ...actions
  };
};

// Create the singleton UI state
export const uiState = createUIState();
