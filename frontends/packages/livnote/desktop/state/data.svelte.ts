import { sendMessage } from "@osvauld/password-manager-common";
import { uiState } from './ui.svelte';
import { listen, emit } from "@tauri-apps/api/event";
import { StoreService } from './storeService';
import { notesInstance } from "../components/notes/notes";
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

export interface UserDetails {
  userId: string,
  deviceId: string,
  username: string,
  publicKey: string,
  deviceKey: string,
}

// Data State class
class DataState {
  // Core data state
  vaults = $state<Vault[]>([{ id: "all", name: "All Vaults" }]);
  currentVault = $state<Vault>({ id: "all", name: "All Vaults" });
  notes = $state<Note[]>([]);
  currentNote = $state<Note | null>(null);
  favoriteSelected = $state<boolean>(false);
  language = $state<string>("en");
  currentView = $state<string>("all");
  isDataLoading = $state<boolean>(false);
  userDetails = $state<UserDetails | null>(null)

  // Derived values for filtering notes - declare as a class property with $derived
  filteredNotes = $derived.by(() => {
    // First filter by favorites if needed
    const favFilter = this.favoriteSelected
      ? this.notes.filter(note => note.favourite)
      : this.notes;
    // Then filter by current vault if not "all"
    return this.currentVault.id === "all"
      ? favFilter
      : favFilter.filter(note => note.folderId === this.currentVault.id);
  });
  /**
   * Get a note by its ID from the cached notes
   * @param id The ID of the note to find
   * @returns The note object if found, null otherwise
   */
  getNoteById(id: string): Note | null {
    const note = this.notes.find(note => note.id === id);
    return note || null;
  }
  // Fetch vaults from backend
  async fetchVaults() {
    try {
      const resp = await sendMessage("getFolder");
      const folderVaults: Vault[] = resp.map(item => ({
        id: item.id || "",
        name: item.name || "",
        description: item.description
      }));

      // Keep "All Vaults" at the top
      this.vaults = [{ id: "all", name: "All Vaults" }, ...folderVaults];
    } catch (error) {
      console.error("Error fetching vaults:", error);
    }
  }

  // Fetch all notes regardless of vault
  async fetchAllNotes() {
    this.isDataLoading = true;
    try {
      const fetchedNotes = await sendMessage("getAllCredentials");
      // Filter for valid notes
      this.notes = fetchedNotes;
      console.log(fetchedNotes);
    } catch (error) {
      console.error("Error fetching notes:", error);
      this.notes = [];
    } finally {
      this.isDataLoading = false;
    }
  }

  // Switch to a different vault
  switchVault(vault: Vault) {
    this.currentVault = vault;
    StoreService.setCurrentVault(vault);
    uiState.toggleNoteViewLayout(false);
  }

  // Switch to a different note
  switchNote(note: Note) {
    this.currentNote = note;
    uiState.toggleNoteViewLayout(true);
    StoreService.setCurrentNoteId(note.id);
    if (note.id) {
      emit("note-change", note.id
      ).catch(error => {
        console.error("Error updating current note:", error);
      });
    }
  }
  updateNoteFavorite(noteId: string) {
    const noteIndex = this.notes.findIndex(n => n.id === noteId);
    if (noteIndex !== -1) {
      this.notes[noteIndex].favourite = !this.notes[noteIndex].favourite;
    }
  }

  // Clear the current note
  clearCurrentNote() {
    this.currentNote = null;
    uiState.toggleNoteViewLayout(false);
    StoreService.setCurrentNoteId(null);
    emit("note-change", null).catch(error => {
      console.error("Error clearing current note:", error);
    });
  }

  // Toggle favorite view filter
  toggleFavoriteView(showFavorites: boolean) {
    this.favoriteSelected = showFavorites;
  }

  // Initialize the state
  async initializeState() {
    this.isDataLoading = true;
    try {
      await this.fetchVaults();
      await this.fetchAllNotes();
      await this.getUserDetails();
      this.setupReactiveUpdates();
      await this.restoreSavedSelections();
    } finally {
      this.isDataLoading = false;
    }
  }

  async getUserDetails() {
    this.userDetails = await sendMessage('getUserDetails');
    const clientId = this.getClientId();
    notesInstance.updateClientId(clientId);
  }

  getClientId(): number {
    if (!this.userDetails) {
      throw new Error("User details not available");
    }
    return parseInt(this.userDetails?.deviceId.substring(0, 8), 16)
  }
  // Restore saved selections from storage
  async restoreSavedSelections() {
    try {
      // Try to get saved vault
      const savedVault = await StoreService.getCurrentVault();

      if (savedVault) {
        // Find if the saved vault exists in the current vaults list
        const vaultExists = this.vaults.some(v => v.id === savedVault.id);

        if (vaultExists) {
          // Apply the saved vault if it exists
          this.currentVault = savedVault;
        }
      }

      // Try to get saved note ID
      const savedNoteId = await StoreService.getCurrentNoteId();

      if (savedNoteId) {
        // Find the note with this ID in the current notes
        const noteExists = this.notes.some(n => n.id === savedNoteId);

        if (noteExists) {
          // Get the fresh note data
          const freshNote = this.notes.find(n => n.id === savedNoteId) || null;

          if (freshNote) {
            this.switchNote(freshNote);
            uiState.toggleNoteViewLayout(true);
          }
        }
      }
    } catch (error) {
      console.error("Error restoring saved selections:", error);
    }
  }

  // Setup reactive updates (to be implemented later)
  setupReactiveUpdates() {
    listen("resource-added", (event) => {

      // Extract the resource from the event payload
      const resource: any = event.payload;

      // Add the new resource to the notes array
      this.notes = [...this.notes, resource];
    });
    listen("resource-update", (event) => {
      console.log("Received resource-update event:", event);

      // Extract the updated resource from the event payload
      const updatedResource: any = event.payload;

      // Find the index of the resource in the notes array
      const resourceIndex = this.notes.findIndex(note => note.id === updatedResource.id);

      if (resourceIndex !== -1) {
        // Create a new array with the updated resource
        this.notes = [
          ...this.notes.slice(0, resourceIndex),
          updatedResource,
          ...this.notes.slice(resourceIndex + 1)
        ];

        // Also update currentNote if it's the same note that was updated
        if (this.currentNote && this.currentNote.id === updatedResource.id) {
          this.currentNote = updatedResource;
        }
      } else {
        // If the resource doesn't exist in the notes array, add it
        console.log("Updated resource not found in notes array, adding it");
        this.notes = [...this.notes, updatedResource];
      }
    });
  }
}

// Create the singleton data state
export const dataState = new DataState();
