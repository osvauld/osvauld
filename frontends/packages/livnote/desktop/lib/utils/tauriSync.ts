
import { emit, listen } from '@tauri-apps/api/event';
import { Doc, DocCollection, Job } from '@blocksuite/store';
import { sendMessage } from '@osvauld/password-manager-common';
import * as Y from 'yjs';
export class TauriSync {
  private collection: DocCollection;
  private deviceId: string;
  private job: Job;

  constructor(collection: DocCollection, deviceId: string) {
    this.collection = collection;
    this.deviceId = deviceId;
    this.job = new Job({ collection });
    console.log('TauriSync: Initializing with deviceId:', deviceId);

    // Set up Tauri event listeners immediately
    this.setupTauriListeners().catch(error => {
      console.error('TauriSync: Error setting up Tauri listeners:', error);
    });

    // Handle doc updates directly
    const currentDoc = this.collection.getDoc('page1');
    if (currentDoc) {
      currentDoc.spaceDoc.on('update', (update) => {
        console.log('TauriSync: Doc update detected:', update);
        this.handleDocUpdate(update);
      });
    }
  }

  private async handleDocUpdate(update: any) {
    try {
      // Prepare update payload
      const updatePayload = {
        sender: this.deviceId,
        update: update
      };

      // Broadcast the update
      await emit('sync-update', updatePayload);
      console.log('TauriSync: Broadcasted doc update');
    } catch (error) {
      console.error('TauriSync: Error handling doc update:', error);
    }
  }

  private async setupTauriListeners() {
    console.log('TauriSync: Setting up Tauri listeners');

    await listen('sync-snapshot', async (event) => {
      console.log('TauriSync: Received sync snapshot event:', event);
      const { sender, snapshot } = event.payload as { sender: string; snapshot: any };
      if (sender === this.deviceId) return;
      await this.loadSnapshot(snapshot);
    });

    await listen('sync-update', async (event) => {
      console.log('TauriSync: Received sync update event:', event);
      const { sender, update } = event.payload as { sender: string; update: any };
      if (sender === this.deviceId) return;
      await this.applyUpdate(update);
    });
  }
  public async sendInitialSnapshot() {
    console.log('TauriSync: Sending initial Y.js state');
    const currentDoc = this.collection.getDoc('page1');

    if (!currentDoc) {
      console.error('TauriSync: No document found with ID page1');
      return;
    }

    try {
      // Get the Y.js document
      const yDoc = currentDoc.spaceDoc;
      // Encode the entire document state
      const encodedState = Y.encodeStateAsUpdate(yDoc);

      await sendMessage("sync-snapshot", encodedState);
      console.log('TauriSync: Emitted Y.js state');
    } catch (error) {
      console.error('TauriSync: Error sending Y.js state:', error);
    }
  }

  private async loadSnapshot(snapshot: Uint8Array) {
    console.log('TauriSync: Loading Y.js state');
    try {
      let doc = this.collection.getDoc('page1');
      if (!doc) {
        // Create the doc if it doesn't exist
        this.collection.createDoc({ id: 'page1' });
        doc = this.collection.getDoc('page1');
      }

      // Apply the Y.js update
      Y.applyUpdate(doc.spaceDoc, snapshot);
      console.log('TauriSync: Successfully loaded Y.js state');
    } catch (error) {
      console.error('TauriSync: Error loading Y.js state:', error);
    }
  }

  private async applyUpdate(update: any) {
    console.log('TauriSync: Applying update');
    try {
      const currentDoc = this.collection.getDoc('page1');
      if (!currentDoc) {
        throw new Error('No document found');
      }

      // Apply the update to the document
      // currentDoc.spaceDoc.applyUpdate(update);
      console.log('TauriSync: Successfully applied update');
    } catch (error) {
      console.error('TauriSync: Error applying update:', error);
    }
  }
}
