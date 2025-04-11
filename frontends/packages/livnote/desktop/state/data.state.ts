// state/data.state.ts
import { sendMessage } from "@osvauld/password-manager-common/utils/helper";

// Define interfaces
export interface Vault {
  id: string;
  name: string;
  description?: string;
}

export interface NoteData {
  title?: string;
  content?: string;
  last_modified?: number;
  last_accessed?: number;
  editor_state?: string | Record<string, unknown>;
  yjs_state?: Uint8Array | number[];
}

export interface Note {
  id: string;
  data: NoteData;
  favourite?: boolean;
  folderId?: string;
}

// Create a root state object for data
const createDataState = () => {
  // Core data state
  const state = {
    vaults: $state<Vault[]>([{ id: "all", name: "All Vaults" }]),
    currentVault: $state<Vault>({ id: "all", name: "All Vaults" }),
    notes: $state<Note[]>([]),
    currentNote: $state<Note | null>(null),
    favoriteSelected: $state<boolean>(false),
    language: $state<string>("en"),
    currentView: $state<string>("all"),
    isDataLoading: $state<boolean>(false),
  };

  // Derived values for filtering notes - this happens client-side
  const filteredNotes = $derived(() => {
    // First filter by favorites if needed
    const favFilter = state.favoriteSelected
      ? state.notes.filter(note => note.favourite)
      : state.notes;

    // Then filter by current vault if not "all"
    return state.currentVault.id === "all"
      ? favFilter
      : favFilter.filter(note => note.folderId === state.currentVault.id);
  });

  // Actions (functions that modify state)
  const actions = {
    async fetchVaults() {
      try {
        const resp = await sendMessage("getFolder");
        const folderVaults: Vault[] = resp.map(item => ({
          id: item.id || "",
          name: item.name || "",
          description: item.description
        }));

        // Keep "All Vaults" at the top
        state.vaults = [{ id: "all", name: "All Vaults" }, ...folderVaults];
      } catch (error) {
        console.error("Error fetching vaults:", error);
      }
    },

    async fetchAllNotes() {
      state.isDataLoading = true;
      try {
        // Fetch ALL notes at once regardless of vault
        const fetchedNotes = await sendMessage("getAllCredentials", {
          favourite: null // null means get all notes regardless of favorite status
        });

        // Filter for valid notes
        state.notes = fetchedNotes.filter(
          cred => cred.data && cred.data.content && cred.data.editor_state
        ).sort((a, b) => {
          const timeA = a.data.last_accessed || a.data.last_modified || 0;
          const timeB = b.data.last_accessed || b.data.last_modified || 0;
          return timeB - timeA;
        });
      } catch (error) {
        console.error("Error fetching notes:", error);
        state.notes = [];
      } finally {
        state.isDataLoading = false;
      }
    },

    switchVault(vault: Vault) {
      state.currentVault = vault;
      // Just switch the vault - filteredNotes will update automatically
      // Clear current note when switching vaults
      if (state.currentNote) {
        state.currentNote = null;
        uiState.noteViewLayout = false;
      }
    },

    switchNote(note: Note) {
      state.currentNote = note;
      uiState.noteViewLayout = true;
    },

    clearCurrentNote() {
      state.currentNote = null;
      uiState.noteViewLayout = false;
    },

    toggleFavoriteView(showFavorites: boolean) {
      state.favoriteSelected = showFavorites;
    },

    async initializeState() {
      state.isDataLoading = true;
      try {
        await actions.fetchVaults();
        await actions.fetchAllNotes();
      } finally {
        state.isDataLoading = false;
      }
    },

    setupReactiveUpdates() {
      // This will be implemented later
      // It will listen for Tauri events and update the state automatically
    }
  };

  return {
    ...state,
    ...actions,
    filteredNotes
  };
};

// Create the singleton data state
export const dataState = createDataState();

// Import from UI state to allow cross-references
import { uiState } from './ui.state';
