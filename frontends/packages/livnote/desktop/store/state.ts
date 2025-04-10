// state/notes.state.ts
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

// Core state
export let vaults = $state<Vault[]>([{ id: "all", name: "All Vaults" }]);
export let currentVault = $state<Vault>({ id: "all", name: "All Vaults" });
export let notes = $state<Note[]>([]);
export let currentNote = $state<Note | null>(null);
export let favoriteSelected = $state<boolean>(false);

// UI state
export let noteViewLayout = $state<boolean>(false);
export let showWelcome = $state<boolean>(true);
export let toastMessage = $state<{ show: boolean, message: string, success: boolean }>({
  show: false, message: "", success: true
});
export let isDataLoading = $state<boolean>(false);

// Derived values for filtering notes - this happens client-side
export const filteredNotes = $derived(() => {
  // First filter by favorites if needed
  const favFilter = favoriteSelected
    ? notes.filter(note => note.favourite)
    : notes;

  // Then filter by current vault if not "all"
  return currentVault.id === "all"
    ? favFilter
    : favFilter.filter(note => note.folderId === currentVault.id);
});

// Actions (functions that modify state)
export async function fetchVaults() {
  try {
    const resp = await sendMessage("getFolder");
    const folderVaults: Vault[] = resp.map(item => ({
      id: item.id || "",
      name: item.name || "",
      description: item.description
    }));

    // Keep "All Vaults" at the top
    vaults = [{ id: "all", name: "All Vaults" }, ...folderVaults];
  } catch (error) {
    console.error("Error fetching vaults:", error);
  }
}

export async function fetchAllNotes() {
  isDataLoading = true;
  try {
    // Fetch ALL notes at once regardless of vault
    const fetchedNotes = await sendMessage("getAllCredentials", {
      favourite: null // null means get all notes regardless of favorite status
    });

    // Filter for valid notes
    notes = fetchedNotes.filter(
      cred => cred.data && cred.data.content && cred.data.editor_state
    ).sort((a, b) => {
      const timeA = a.data.last_accessed || a.data.last_modified || 0;
      const timeB = b.data.last_accessed || b.data.last_modified || 0;
      return timeB - timeA;
    });
  } catch (error) {
    console.error("Error fetching notes:", error);
    notes = [];
  } finally {
    isDataLoading = false;
  }
}

export function switchVault(vault: Vault) {
  currentVault = vault;
  // Just switch the vault - filteredNotes will update automatically
  // Clear current note when switching vaults
  if (currentNote) {
    currentNote = null;
    noteViewLayout = false;
  }
}

export function switchNote(note: Note) {
  currentNote = note;
  noteViewLayout = true;
}

export function clearCurrentNote() {
  currentNote = null;
  noteViewLayout = false;
}

// Toggle favorite/all notes view (UI filter only)
export function toggleFavoriteView(showFavorites: boolean) {
  favoriteSelected = showFavorites;
}

// Initialize the state
export async function initializeState() {
  isDataLoading = true;
  try {
    await fetchVaults();
    await fetchAllNotes();
  } finally {
    isDataLoading = false;
  }
}

// Setup Tauri event listeners for reactive updates
export function setupReactiveUpdates() {
  // This will be implemented later
  // It will listen for Tauri events and update the state automatically
  // For now, we'll keep the UI-driven approach
}
