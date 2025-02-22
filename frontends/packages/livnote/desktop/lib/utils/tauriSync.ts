import { emit } from '@tauri-apps/api/event';
import { Doc, DocCollection, Job } from '@blocksuite/store';
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
    this.initializeDocument();
  }

  private initializeDocument() {
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
          console.log('TauriSync: Local doc update detected, sending update');
          this.handleDocUpdate(update);
        }
      });
    }
  }

  private async handleDocUpdate(update: Uint8Array) {
    try {
      // Convert the update to a regular array for serialization
      const updateArray = Array.from(update);
      console.log('TauriSync: Sending update, length:', updateArray.length);

      // Emit the update event
      await emit('sync-update', JSON.stringify(updateArray));
      console.log('TauriSync: Successfully sent update');
    } catch (error) {
      console.error('TauriSync: Error sending update:', error);
    }
  }

  public async sendInitialSnapshot() {
    console.log('TauriSync: Sending initial Y.js state');
    if (!this.currentDoc) {
      console.error('TauriSync: No document available to create snapshot');
      return;
    }

    try {
      const encodedState = Y.encodeStateAsUpdate(this.currentDoc.spaceDoc);
      const stateArray = Array.from(encodedState);
      console.log('TauriSync: Sending snapshot, length:', stateArray.length);

      await emit('sync-snapshot', JSON.stringify(stateArray));
      console.log('TauriSync: Successfully sent initial snapshot');
    } catch (error) {
      console.error('TauriSync: Error sending initial snapshot:', error);
    }
  }
}
