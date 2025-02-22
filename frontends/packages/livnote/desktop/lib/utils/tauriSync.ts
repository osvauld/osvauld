import { emit, listen } from '@tauri-apps/api/event';
import { Doc, DocCollection, Job } from '@blocksuite/store';
import { sendMessage } from '@osvauld/password-manager-common';
import * as Y from 'yjs';

export class TauriSync {
  private collection: DocCollection;
  private deviceId: string;
  private job: Job;
  private currentDoc: Doc | null;

  constructor(collection: DocCollection, deviceId: string) {
    this.collection = collection;
    this.deviceId = deviceId;
    this.job = new Job({ collection });
    this.currentDoc = null;

    console.log('TauriSync: Initializing with deviceId:', deviceId);

    // Initialize the document and set up listeners
    this.initializeDocument();
    this.setupTauriListeners().catch(error => {
      console.error('TauriSync: Error setting up Tauri listeners:', error);
    });
  }

  private initializeDocument() {
    // Get or create the document
    this.currentDoc = this.collection.getDoc('page1');
    if (!this.currentDoc) {
      this.collection.createDoc({ id: 'page1' });
      this.currentDoc = this.collection.getDoc('page1');
    }

    if (this.currentDoc) {
      // Set up update handler for the document
      this.currentDoc.spaceDoc.on('update', (update: Uint8Array, origin: any) => {
        // Only broadcast updates that originated from this device
        if (origin !== 'remote') {
          console.log('TauriSync: Local doc update detected');
          this.handleDocUpdate(update);
        }
      });
    }
  }

  private async handleDocUpdate(update: Uint8Array) {
    try {
      // Convert the update to a regular array for serialization
      const updateArray = Array.from(update);

      // Broadcast the update
      await emit('sync-update', JSON.stringify(updateArray));
      console.log('TauriSync: Broadcasted doc update');
    } catch (error) {
      console.error('TauriSync: Error handling doc update:', error);
    }
  }

  private async setupTauriListeners() {
    console.log('TauriSync: Setting up Tauri listeners');

    // Listen for sync snapshots
    await listen('sync-snapshot-be', async (event) => {
      console.log('TauriSync: Received sync snapshot event');
      try {
        const binaryData = new Uint8Array(event.payload as number[]);
        await this.loadSnapshot(binaryData);
      } catch (error) {
        console.error('TauriSync: Error processing sync snapshot:', error);
      }
    });

    // Listen for sync updates
    await listen('sync-update-be', async (event) => {
      console.log('TauriSync: Received sync update event');
      try {
        // Parse the JSON string back to an array and convert to Uint8Array
        const updateArray = JSON.parse(event.payload as string);
        const update = new Uint8Array(updateArray);
        await this.applyUpdate(update);
      } catch (error) {
        console.error('TauriSync: Error processing sync update:', error);
      }
    });
  }

  public async sendInitialSnapshot() {
    console.log('TauriSync: Sending initial Y.js state');

    if (!this.currentDoc) {
      console.error('TauriSync: No document available to create snapshot');
      return;
    }

    try {
      // Encode the entire document state
      const encodedState = Y.encodeStateAsUpdate(this.currentDoc.spaceDoc);

      // Convert to regular array for serialization
      const stateArray = Array.from(encodedState);

      await sendMessage("sendSnapshot", stateArray);
      console.log('TauriSync: Successfully sent initial snapshot');
    } catch (error) {
      console.error('TauriSync: Error sending initial snapshot:', error);
    }
  }

  private async loadSnapshot(snapshot: Uint8Array) {
    console.log('TauriSync: Loading snapshot');
    if (!this.currentDoc) {
      console.error('TauriSync: No document available to load snapshot');
      return;
    }

    try {
      // Apply the snapshot to the Y.js document
      Y.applyUpdate(this.currentDoc.spaceDoc, snapshot, 'remote');
      console.log('TauriSync: Successfully loaded snapshot');
    } catch (error) {
      console.error('TauriSync: Error loading snapshot:', error);
    }
  }

  private async applyUpdate(update: Uint8Array) {
    console.log('TauriSync: Applying update');
    if (!this.currentDoc) {
      console.error('TauriSync: No document available to apply update');
      return;
    }

    try {
      // Apply the update to the Y.js document
      Y.applyUpdate(this.currentDoc.spaceDoc, update, 'remote');
      console.log('TauriSync: Successfully applied update');
    } catch (error) {
      console.error('TauriSync: Error applying update:', error);
    }
  }
}
