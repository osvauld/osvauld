import { sendMessage } from "@osvauld/password-manager-common";
import { uiState } from './ui.svelte';
import { listen, emit } from "@tauri-apps/api/event";
import { StoreService } from './storeService';
import { notesInstance } from "../components/notes/notes";
import { applyYjsUpdates } from "../components/notes/documentUtils";
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
  private _unlisteners: Array<() => void> = [];

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
      // Run these operations in parallel
      await Promise.all([
        this.fetchVaults(),
        this.fetchAllNotes(),
        this.getUserDetails(),
        this.setupReactiveUpdates()
      ]);

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
  async handleAwarenessUpdates(event: any) {
    try {
      const { resource_id, connection_id, updates, client_id } = event.payload;

      // Find the note with this resource ID
      const note = this.getNoteById(resource_id);

      if (!note) {
        console.warn(`Note with ID ${resource_id} not found for awareness updates`);
        return;
      }

      // Convert the updates array to Uint8Array for YJS
      const updatesArray = new Uint8Array(updates);

      // Parse client_id to number for sender identification
      const senderId = parseInt(client_id, 10);

      if (notesInstance) {
        // Apply the awareness updates to the current editor
        console.log(`Applying awareness updates from client ${senderId} for resource ${resource_id}`);
        notesInstance.applyAwarenessUpdate(updatesArray, senderId);
      } else {
        console.warn("Notes instance not available for awareness updates");
      }
    } catch (error) {
      console.error("Error handling awareness-updates:", error);
    }
  }

  async handleLiveUpdates(event: any) {
    try {
      const { resource_id, updates, client_id } = event.payload;

      // Check if this is for the current note
      if (!this.currentNote || this.currentNote.id !== resource_id) {
        console.log(`Received live update for non-active note: ${resource_id}`);
        return;
      }

      // Convert the updates array to Uint8Array for YJS
      const updatesArray = new Uint8Array(updates);

      // Parse client_id to number for sender identification
      const senderId = parseInt(client_id, 10);

      if (notesInstance) {
        // Apply the live updates directly to the current editor
        console.log(`Applying live updates from client ${senderId} to current editor for resource ${resource_id}`);
        notesInstance.applyUpdate(updatesArray, senderId);

        // No need to save here - the editor handles auto-save
      } else {
        console.warn("Notes instance not available for live updates");
      }
    } catch (error) {
      console.error("Error handling live-updates:", error);
    }
  }



  async setupReactiveUpdates() {
    // Clear any existing unlisteners first
    this._unlisteners = [];

    // Each listen() returns a Promise that resolves to an unlisten function
    const resourceAddedUnlisten = await listen("resource-added", this.handleResourceAdded.bind(this));
    const resourceUpdateUnlisten = await listen("resource-update", this.handleResourceUpdate.bind(this));
    const documentUpdatesUnlisten = await listen("document-updates", this.handleDocumentUpdates.bind(this));
    const awarenessUpdatesUnlisten = await listen("awareness-updates", this.handleAwarenessUpdates.bind(this));
    const liveUpdatesUnlisten = await listen("live-updates", this.handleLiveUpdates.bind(this));

    // Store all the unlisten functions
    this._unlisteners.push(
      resourceAddedUnlisten,
      resourceUpdateUnlisten,
      documentUpdatesUnlisten,
      awarenessUpdatesUnlisten,
    );
  }
  cleanupReactiveUpdates() {
    // The listen function returns an unlisten function
    if (this._unlisteners) {
      for (const unlisten of this._unlisteners) {
        unlisten();
      }
      this._unlisteners = [];
    }
  }

  handleResourceAdded(event: any) {
    const resource = event.payload;
    this.notes = [...this.notes, resource];
  }

  handleResourceUpdate(event: any) {
    const updatedResource = event.payload;
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
      this.notes = [...this.notes, updatedResource];
    }
  }

  async handleDocumentUpdates(event: any) {
    try {
      const { resource_id, updates } = event.payload;

      // Find the note with this resource ID
      const note = this.getNoteById(resource_id);

      if (!note) {
        console.warn(`Note with ID ${resource_id} not found for updates`);
        return;
      }

      // Convert the updates array to Uint8Array for YJS
      const updatesArray = new Uint8Array(updates);

      if (notesInstance) {
        // If this is the current note, apply the updates directly to the editor
        console.log("Applying updates directly to current editor");
        notesInstance.applyUpdate(updatesArray, 0);
        // The editor will save the note automatically
      } else {
        // For non-current notes, use our utility function to apply updates to stored state
        await this.updateNoteYjsState(note, updatesArray);
      }
    } catch (error) {
      console.error("Error handling document-updates:", error);
    }
  }
  // Update the YJS state of a note and save it

  async updateNoteYjsState(note: Note, updates: Uint8Array) {
    try {
      console.log(`Starting YJS state update for note ${note.id}, update size: ${updates.length} bytes`);

      // Get the current YJS state
      const currentYjsState: any = note.data.yjs_state;
      console.log(`Current YJS state size: ${currentYjsState ?
        (Array.isArray(currentYjsState) ? currentYjsState.length : currentYjsState.byteLength) : 0} bytes`);

      // Apply the updates to get the new state with content
      const { yjs_state: newYjsState, content, editor_state } = applyYjsUpdates(currentYjsState, updates);
      // Verify state changed
      const changed = !currentYjsState ||
        JSON.stringify(newYjsState) !== JSON.stringify(Array.from(currentYjsState));
      if (!changed) {
        console.warn(`No change detected after applying updates to note ${note.id}`);
      } else {
        console.log(`YJS state changed for note ${note.id}`);
      }

      // Create a timestamp for the modification
      const timestamp = Date.now();

      // Create a clone of the note data with the updated states
      const updatedData = {
        ...note.data,
        yjs_state: newYjsState,
        content: content,               // Add the extracted content
        editor_state: editor_state,     // Add the updated editor state
        last_modified: timestamp
      };

      // Send to backend for persistence
      const response = await sendMessage("updateCredential", {
        id: note.id,
        data: JSON.stringify({
          ...updatedData,
          yjs_state: Array.from(newYjsState),  // Convert to array for JSON serialization
        })
      });

      console.log(`Backend update response for note ${note.id}:`, response);

      // Important: Emit completion event even for non-active notes
      await emit('resource-update-complete', { id: note.id });

      // Update local state to reflect the changes
      const noteIndex = this.notes.findIndex(n => n.id === note.id);
      if (noteIndex !== -1) {
        this.notes[noteIndex].data = updatedData;
        console.log(`Updated local state for note ${note.id}`);

        // Force reactive update to make sure the UI reflects the changes
        this.notes = [...this.notes];
      }

      return true;
    } catch (error) {
      console.error(`Error updating YJS state for note ${note.id}:`, error);
      return false;
    }
  }
}


// Create the singleton data state
export const dataState = new DataState();
