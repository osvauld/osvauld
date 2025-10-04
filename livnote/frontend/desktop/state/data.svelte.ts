import { sendMessage } from "../utils/helper";
import { uiState } from './ui.svelte';
import { listen, emit } from "@tauri-apps/api/event";
import { StoreService } from './storeService';
import { createEmptyNoteContent } from "../components/notes/documentUtils";
import type { Note, NotePreview, Collaborator } from "../types/notes.types";
import { NotesCoordinator } from "../components/notes/notesCoordinator";
export interface Vault {
  id: string;
  name: string;
  description?: string;
  default?: boolean;
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
  vaults = $state<Vault[]>([{ id: "all", name: "All Vaults" }]);
  currentVault = $state<Vault>({ id: "all", name: "All Vaults" });
  notes = $state<NotePreview[]>([]);
  private notesCoordinator: NotesCoordinator | null = null;
  currentNoteData = $state<Note | null>(null);
  favoriteSelected = $state<boolean>(false);
  language = $state<string>("en");
  currentView = $state<string>("all");
  isDataLoading = $state<boolean>(false);
  userDetails = $state<UserDetails | null>(null)
  currentNoteId = $state<string | null>(null);
  private _unlisteners: Array<() => void> = [];
  sharedUsers = $state([]);
  sharedFolderUsers = $state([]);
  collaborators = $state<Collaborator[]>([]);
  clientId: number = 0;
  currentNoteTitle = $state<string>("");
  searchResults = $state<string[]>([]);
  isSearchActive = $state<boolean>(false);

  getNotesCoordinator(): NotesCoordinator | null {
    return this.notesCoordinator;
  }
  getCurrentNoteId(): string | null {
    return this.currentNoteId;
  }
  setCurrentNoteId(noteId: string | null) {
    this.currentNoteId = noteId;
  }

  setCurrentNoteData(note: Note | null) {
    this.currentNoteData = note;
  }

  getCurrentNoteData(): Note | null {
    return this.currentNoteData;
  }

  setCurrentNoteTitle(title: string) {
    this.currentNoteTitle = title;
  }
  private baseFilteredNotes = $derived.by(() => {
    const favFilter = this.favoriteSelected
      ? this.notes.filter(note => note.favourite)
      : this.notes;
    
    const result = this.currentVault.id === "all"
      ? favFilter
      : favFilter.filter(note => note.folderId === this.currentVault.id);
    
    // Debug: Uncomment for troubleshooting folder filtering
    // console.log(`Filtering notes for vault: ${this.currentVault.id}, total notes: ${this.notes.length}, filtered: ${result.length}`);
    
    return result;
  });

  // Now filteredNotes applies search on top of base filtering and sorts by lastModified
  filteredNotes = $derived.by(() => {
    let result;
    if (!this.isSearchActive || this.searchResults.length === 0) {
      result = this.baseFilteredNotes;
    } else {
      // Apply search filter to base filtered notes
      result = this.baseFilteredNotes.filter(note => this.searchResults.includes(note.id));
    }

    // Sort by lastModified in descending order (most recent first)
    return result.slice().sort((a, b) => {
      const aTime = a.lastModified || 0;
      const bTime = b.lastModified || 0;
      return bTime - aTime; // Descending order
    });
  });
  setSearchResults(noteIds: string[]) {
    this.searchResults = noteIds;
    this.isSearchActive = noteIds.length > 0;
  }

  // Method to clear search
  clearSearch() {
    this.searchResults = [];
    this.isSearchActive = false;
  }
  /**
   * Get a note by its ID from the cached notes
   * @param id The ID of the note to find
   * @returns The note object if found, null otherwise
   */
  getNoteById(id: string): NotePreview | null {
    const note = this.notes.find(note => note.id === id);
    return note || null;
  }

  async fetchVaults() {
    try {
      const resp = await sendMessage("getFolder");
      const folderVaults: Vault[] = resp.map((item: any) => ({
        id: item.id || "",
        name: item.name || "",
        description: item.description,
        default: item.default
      }));
      this.vaults = [{ id: "all", name: "All Vaults" }, ...folderVaults];
    } catch (error) {
      console.error("Error fetching vaults:", error);
    }
  }

  updateCollaborators(newCollaborators: Collaborator[]) {
    this.collaborators = newCollaborators;
  }

  async fetchAllNotes(selectedNotedId?: string) {
    this.isDataLoading = true;
    this.notes = [];
    try {
      const response = await sendMessage("emitAllResources", selectedNotedId);
      if (response) {
        this.setCurrentNoteData(response);
      }
    } catch (error) {
      console.error("Error fetching notes:", error);
      this.notes = [];
    } finally {
      this.isDataLoading = false;
    }
  }

  async switchVault(vault: Vault) {
    this.currentVault = vault;
    StoreService.setCurrentVault(vault);
    uiState.toggleNoteViewLayout(false);
    // Reset favorite selection when switching vaults
    this.favoriteSelected = false;
    this.fetchSharedFolderUsers(vault.id);
    
    // Ensure notes are available when switching vaults
    // Only refetch if we have no notes or if we're not currently loading
    if (this.notes.length === 0 && !this.isDataLoading) {
      await this.fetchAllNotes();
    }
  }
  async fetchSharedFolderUsers(folderId: string) {
    try {
      // Don't fetch shared users for "all" vault
      if (folderId === "all") {
        this.sharedFolderUsers = [];
        return;
      }

      const sharedUsers = await sendMessage("getSharedFolderUsers", { folderId });
      console.log(sharedUsers);
      this.sharedFolderUsers = sharedUsers || [];
    } catch (error) {
      console.error("Error fetching shared folder users:", error);
      this.sharedFolderUsers = [];
    }
  }
  async addNote() {
    const noteContent = createEmptyNoteContent(this.clientId, this.userDetails?.username);
    const note = await sendMessage("addCredential", {
      resourcePayload: JSON.stringify(noteContent),
      folderId: this.currentVault.id,
      resourceType: "notes"
    });
    
    // Use switchNote to ensure consistent state management including folder highlighting
    await this.switchNote(note.id);
  }

  getNoteTitle(): string {
    return this.getNotesCoordinator()?.getCurrentTitle() || "Untitled";
  }

  async switchNote(noteId: string | null) {
    emit("note-change", noteId
    ).catch(error => {
      uiState.clearAllLoadingStates();
      console.error("Error updating current note:", error);
    });
    if (noteId) {
      uiState.setNoteFetching(true);
      uiState.setEditorLoading(false);
      uiState.toggleNoteViewLayout(true);
      const note = await sendMessage("getCredential", { resourceId: noteId })
      this.setCurrentNoteData(note);
      this.setCurrentNoteId(noteId);
      uiState.setNoteFetching(false);
      StoreService.setCurrentNoteId(noteId);
      
      // Update currentVault to match the note's folder for correct folder highlighting
      const notePreview = this.getNoteById(noteId);
      if (notePreview?.folderId) {
        const folder = this.vaults.find(v => v.id === notePreview.folderId);
        if (folder) {
          this.currentVault = folder;
          StoreService.setCurrentVault(folder);
        }
      }
    } else {
      dataState.clearCurrentNote();
    }

  }

  updateNoteFavorite(noteId: string) {
    const noteIndex = this.notes.findIndex(n => n.id === noteId);
    if (noteIndex !== -1) {
      this.notes[noteIndex].favourite = !this.notes[noteIndex].favourite;
    }
  }

  updateNoteTitle(noteId: string, newTitle: string) {
    const noteIndex = this.notes.findIndex(n => n.id === noteId);
    if (noteIndex !== -1) {
      this.notes[noteIndex].title = newTitle;
    }
  }

  updateNoteLastModified(noteId: string, timestamp: number) {
    const noteIndex = this.notes.findIndex(n => n.id === noteId);
    if (noteIndex !== -1) {
      this.notes[noteIndex].lastModified = timestamp;
    }
  }

  clearCurrentNote() {
    this.setCurrentNoteId(null);
    this.setCurrentNoteData(null);
    this.setCurrentNoteTitle("");
    StoreService.setCurrentNoteId(null);
  }

  toggleFavoriteView(showFavorites: boolean) {
    this.favoriteSelected = showFavorites;
  }
  private createCoordinator() {
    if (this.notesCoordinator) {
      this.notesCoordinator.destroy();
    }

    if (!this.userDetails) {
      throw new Error("User details not available for coordinator creation");
    }

    const userInfo = {
      name: this.userDetails.username,
      color: this.generateUserColor(),
      id: this.clientId,
      userId: this.userDetails.userId,
    };
    this.notesCoordinator = new NotesCoordinator({
      onCollaborationUpdate: async (update, docType) => {
        if (!this.getCurrentNoteId()) return;
        await emit("sync-update", {
          update: Array.from(update),
          clientID: this.clientId,
          resource_id: this.getCurrentNoteId(),
          doc_type: docType,
        });
      },
      onAwarenessUpdate: async (changes) => {
        if (!this.getCurrentNoteId()) return;

        await emit("awareness-update", {
          update: Array.from(changes),
          clientID: this.clientId,
          resource_id: this.getCurrentNoteId(),
        });
      },
      userInfo,
    },);
  }

  async initializeState() {
    this.isDataLoading = true;

    // Clear any existing state and event listeners first
    this.clearAllState();

    const savedNoteId = await StoreService.getCurrentNoteId();
    await Promise.all([
      this.fetchVaults(),
      this.getUserDetails(),
      this.setupReactiveUpdates()
    ]);
    this.createCoordinator();
    await this.restoreSavedSelections();
    this.isDataLoading = false;
    if (savedNoteId) {
      this.fetchAllNotes(savedNoteId)
    } else {
      this.fetchAllNotes();
    }
  }

  // Add a method to clear all state when logging out
  clearAllState() {
    // Clear notes state
    this.notes = [];
    this.currentNoteId = null;
    this.currentNoteData = null;
    this.currentNoteTitle = "";
    this.favoriteSelected = false;
    this.currentView = "all";
    this.sharedUsers = [];
    this.sharedFolderUsers = [];
    this.collaborators = [];

    // Clean up event listeners
    this.cleanupReactiveUpdates();

    // Clean up coordinator
    if (this.notesCoordinator) {
      this.notesCoordinator.destroy();
      this.notesCoordinator = null;
    }
  }
  private generateUserColor(): string {
    const colors = [
      "#FF5630",
      "#FFAB00",
      "#36B37E",
      "#00B8D9",
      "#6554C0",
      "#FF7452",
    ];
    return colors[Math.floor(Math.random() * colors.length)];
  }

  async getUserDetails() {
    this.userDetails = await sendMessage('getUserDetails');
    this.clientId = this.getClientId();
  }

  getClientId(): number {
    if (!this.userDetails) {
      throw new Error("User details not available");
    }
    const decoded = atob(this.userDetails.deviceId);
    const bytes = new Uint8Array(decoded.length);
    for (let i = 0; i < decoded.length; i++) {
      bytes[i] = decoded.charCodeAt(i);
    }
    const hex = Array.from(bytes.slice(0, 4))
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');

    return parseInt(hex, 16);
  }

  async restoreSavedSelections() {
    try {
      // Always start with All Notes folder focused, regardless of saved state
      const homeVault = this.vaults.find(v => v.id === "all");
      if (homeVault) {
        this.currentVault = homeVault;
        // Update saved state to reflect All Notes selection
        StoreService.setCurrentVault(homeVault);
      }
    } catch (error) {
      console.error("Error restoring saved selections:", error);
    }
  }
  async handleAwarenessUpdates(event: any) {
    try {
      const { resource_id, connection_id, updates, client_id } = event.payload;
      const updatesArray = new Uint8Array(updates);
      const senderId = parseInt(client_id, 10);
      const coordinator = this.getNotesCoordinator();
      coordinator?.applyAwarenessUpdate(updatesArray, senderId);
    } catch (error) {
      console.error("Error handling awareness-updates:", error);
    }
  }

  async handleLiveUpdates(event: any) {
    try {
      const { resource_id, updates, client_id, doc_type } = event.payload;
      if (!this.currentNoteId || this.currentNoteId !== resource_id) {
        return;
      }
      const updatesArray = new Uint8Array(updates);
      const senderId = parseInt(client_id, 10);
      const coordinator = this.getNotesCoordinator()
      coordinator?.applyRemoteUpdate(updatesArray, senderId, doc_type);
    } catch (error) {
      console.error("Error handling live-updates:", error);
    }
  }

  async handleSharedUsersUpdate(event: any) {
    this.sharedUsers = event.payload;
  }

  async handleResourceAddedNotification(event: any) {
    console.log(event.payload);
    let message = `${event.payload.username} shared a resource`
    uiState.showToast(message, true);
  }

  async handleFoldersAddedUpdate(event: any) {
    console.log(event.payload);
    this.vaults = [...this.vaults, ...event.payload.folders];
  }

  async setupReactiveUpdates() {
    this._unlisteners = [];

    const resourceAddedUnlisten = await listen("resource-added", this.handleResourceAdded.bind(this));
    const resourceUpdateUnlisten = await listen("resource-update", this.handleResourceUpdate.bind(this));
    const documentUpdatesUnlisten = await listen("document-updates", this.handleDocumentUpdates.bind(this));
    const awarenessUpdatesUnlisten = await listen("awareness-updates", this.handleAwarenessUpdates.bind(this));
    const liveUpdatesUnlisten = await listen("live-updates", this.handleLiveUpdates.bind(this));
    const sharedUsersUpdate = await listen("shared-users-update", this.handleSharedUsersUpdate.bind(this));
    const foldersAddedUnlisten = await listen(
      "folders-added-notification", this.handleFoldersAddedUpdate.bind(this))

    const resourceAddedNotification = await listen(
      "resource-added-notification",
      this.handleResourceAddedNotification.bind(this));


    this._unlisteners.push(
      resourceAddedUnlisten,
      resourceUpdateUnlisten,
      documentUpdatesUnlisten,
      awarenessUpdatesUnlisten,
      liveUpdatesUnlisten,
      sharedUsersUpdate,
      resourceAddedNotification,
      foldersAddedUnlisten,
    );
  }

  cleanupReactiveUpdates() {
    if (this._unlisteners) {
      for (const unlisten of this._unlisteners) {
        unlisten();
      }
      this._unlisteners = [];
    }
  }

  handleResourceAdded(event: any) {
    const notePreview = event.payload;
    // Check if note already exists to prevent duplicates
    const existingNote = this.notes.find(note => note.id === notePreview.id);
    if (!existingNote) {
      this.notes = [...this.notes, notePreview];
    }
  }

  handleResourceUpdate(event: any) {
    const updatedResourcePreview = event.payload;
    const resourceIndex = this.notes.findIndex(note => note.id === updatedResourcePreview.id);

    if (resourceIndex !== -1) {
      const existingNote = this.notes[resourceIndex];
      
      // Preserve local lastModified if it's newer than the backend version
      // This handles the case where we just saved locally but backend hasn't updated yet
      if (existingNote.lastModified && updatedResourcePreview.lastModified) {
        updatedResourcePreview.lastModified = Math.max(
          existingNote.lastModified,
          updatedResourcePreview.lastModified
        );
      } else if (existingNote.lastModified && !updatedResourcePreview.lastModified) {
        updatedResourcePreview.lastModified = existingNote.lastModified;
      }
      
      this.notes = [
        ...this.notes.slice(0, resourceIndex),
        updatedResourcePreview,
        ...this.notes.slice(resourceIndex + 1)
      ];
    } else {
      // Only add if it doesn't already exist
      const existingNote = this.notes.find(note => note.id === updatedResourcePreview.id);
      if (!existingNote) {
        this.notes = [...this.notes, updatedResourcePreview];
      }
    }
  }

  async saveNote(noteId: string) {
    const coordinator = this.getNotesCoordinator();
    if (!coordinator) {
      throw new Error("Coordinator not available");
    }
    const noteContent = coordinator.saveNote();
    let stateVectors = coordinator.getStateVectors();
    
    // Update lastModified timestamp in the notes array
    if (noteContent.last_modified) {
      this.updateNoteLastModified(noteId, noteContent.last_modified);
    }
    
    await sendMessage("updateCredential", {
      id: noteId,
      data: JSON.stringify(noteContent),
    });
    emit("resource-update-complete", { id: noteId, state_vectors: stateVectors });
    uiState.setNoteSaved(true);

    setTimeout(() => {
      uiState.setNoteSaved(false);
    }, 1500);
  }

  async handleDocumentUpdates(event: any) {
    try {
      const { resource_id, updates } = event.payload;
      if (resource_id == this.currentNoteId) {
        const updatesJson = JSON.parse(updates)
        const imageUpdates = updatesJson.image_state.updates;
        const documentUpdates = updatesJson.main_doc.updates;
        const imageUpdateArray = new Uint8Array(imageUpdates);
        const documentUpdateArray = new Uint8Array(documentUpdates);
        const coordinator = this.getNotesCoordinator();
        coordinator?.applyRemoteUpdate(imageUpdateArray, updatesJson.client_id, "images");
        coordinator?.applyRemoteUpdate(documentUpdateArray, updatesJson.client_id, "main");
      }
    } catch (error) {
      console.error("Error handling document-updates:", error);
    }
  }

}

export const dataState = new DataState();
