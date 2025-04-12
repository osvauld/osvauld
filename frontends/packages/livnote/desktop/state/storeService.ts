import { LazyStore } from '@tauri-apps/plugin-store';
import type { Vault, Note } from './data.svelte';

// Create a single LazyStore instance for application settings
const appStore = new LazyStore('app_settings.json');

// Store keys
const CURRENT_VAULT_KEY = 'currentVault';
const CURRENT_NOTE_KEY = 'currentNote';

export const StoreService = {
  // Vault operations
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

  // Note operations
  getCurrentNote: async (): Promise<Note | null> => {
    try {
      const note = await appStore.get<Note>(CURRENT_NOTE_KEY);
      return note !== undefined ? note : null;
    } catch (error) {
      console.error('Error getting current note from store:', error);
      return null;
    }
  },

  setCurrentNote: async (note: Note | null): Promise<void> => {
    try {
      await appStore.set(CURRENT_NOTE_KEY, note);
      await appStore.save(); // Ensure it's saved to disk
    } catch (error) {
      console.error('Error saving current note to store:', error);
    }
  },

  // Clear specific selections
  clearSelections: async (): Promise<void> => {
    try {
      await appStore.delete(CURRENT_VAULT_KEY);
      await appStore.delete(CURRENT_NOTE_KEY);
      await appStore.save();
    } catch (error) {
      console.error('Error clearing selections from store:', error);
    }
  }
};
