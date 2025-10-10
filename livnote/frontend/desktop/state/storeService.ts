import { LazyStore } from '@tauri-apps/plugin-store';
import type { Vault } from './data.svelte';

const appStore = new LazyStore('app_settings.json');

const CURRENT_VAULT_KEY = 'currentVault';
const CURRENT_NOTE_ID_KEY = 'currentNoteId';
const STARTER_NOTES_CREATED_KEY = 'starterNotesCreated';

export const StoreService = {
  getCurrentVault: async (): Promise<Vault | null> => {
    try {
      const vault = await appStore.get<Vault>(CURRENT_VAULT_KEY);
      return vault !== undefined ? vault : null;
    } catch (error) {
      console.error('Error getting current vault from store:', error);
      return null;
    }
  },

  setCurrentVault: async (vault: Vault): Promise<void> => {
    try {
      await appStore.set(CURRENT_VAULT_KEY, vault);
      await appStore.save(); // Ensure it's saved to disk
    } catch (error) {
      console.error('Error saving current vault to store:', error);
    }
  },

  getCurrentNoteId: async (): Promise<string | null> => {
    try {
      const noteId = await appStore.get<string>(CURRENT_NOTE_ID_KEY);
      return noteId !== undefined ? noteId : null;
    } catch (error) {
      console.error('Error getting current note ID from store:', error);
      return null;
    }
  },

  setCurrentNoteId: async (noteId: string | null): Promise<void> => {
    try {
      await appStore.set(CURRENT_NOTE_ID_KEY, noteId);
      await appStore.save(); // Ensure it's saved to disk
    } catch (error) {
      console.error('Error saving current note ID to store:', error);
    }
  },

  clearSelections: async (): Promise<void> => {
    try {
      await appStore.delete(CURRENT_VAULT_KEY);
      await appStore.delete(CURRENT_NOTE_ID_KEY);
      await appStore.save();
    } catch (error) {
      console.error('Error clearing selections from store:', error);
    }
  },

  getStarterNotesCreated: async (): Promise<boolean> => {
    try {
      const created = await appStore.get<boolean>(STARTER_NOTES_CREATED_KEY);
      return created === true;
    } catch (error) {
      console.error('Error getting starter notes flag from store:', error);
      return false;
    }
  },

  setStarterNotesCreated: async (created: boolean): Promise<void> => {
    try {
      await appStore.set(STARTER_NOTES_CREATED_KEY, created);
      await appStore.save();
    } catch (error) {
      console.error('Error saving starter notes flag to store:', error);
    }
  }
};
