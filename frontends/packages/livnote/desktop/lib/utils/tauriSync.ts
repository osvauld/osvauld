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

    // Convert Map to Array before mapping
    const docIds = Array.from(collection.docs.values()).map(doc => doc.id);
    console.log('TauriSync: Collection info:', {
      docIds,
      id: collection.id,
      schema: collection.schema
    });

    this.initializeDocument();
  }

  private initializeDocument() {
    console.log('TauriSync: Initializing document');
    this.currentDoc = this.collection.getDoc('page1');

    if (!this.currentDoc) {
      console.log('TauriSync: Document not found, creating new one');
      this.collection.createDoc({ id: 'page1' });
      this.currentDoc = this.collection.getDoc('page1');
    }

    if (this.currentDoc) {
      console.log('TauriSync: Document initialized:', {
        id: this.currentDoc.id,
        isEmpty: this.currentDoc.isEmpty,
        meta: this.currentDoc.meta
      });

      // Set up update handler for the document
      this.currentDoc.spaceDoc.on('update', (update: Uint8Array, origin: unknown) => {
        console.log('TauriSync: Update detected:', {
          updateSize: update.length,
          origin,
          isRemote: origin === 'remote'
        });

        // Only broadcast updates that originated from this device
        if (origin !== 'remote') {
          console.log('TauriSync: Local doc update detected, sending update');
          this.handleDocUpdate(update);
        }
      });

      // Log whenever the doc changes
      this.currentDoc.spaceDoc.on('afterTransaction', (transaction: Y.Transaction) => {
        console.log('TauriSync: Document transaction:', {
          origin: transaction.origin,
          changed: transaction.changed.size > 0,
          deletedNodes: transaction.deletedNodes.size
        });
      });
    }
  }

  private async handleDocUpdate(update: Uint8Array) {
    try {
      // Convert the update to a regular array for serialization
      const updateArray = Array.from(update);
      console.log('TauriSync: Preparing to send update:', {
        length: updateArray.length,
        deviceId: this.deviceId
      });

      // Emit the update event with different name to match backend
      await emit('sync-update', updateArray);
      console.log('TauriSync: Successfully sent update, length:', updateArray.length);
    } catch (error) {
      console.error('TauriSync: Error sending update:', error);
    }
  }

  public async sendInitialSnapshot() {
    console.log('TauriSync: Preparing to send initial Y.js state');
    if (!this.currentDoc) {
      console.error('TauriSync: No document available to create snapshot');
      return;
    }

    try {
      const encodedState = Y.encodeStateAsUpdate(this.currentDoc.spaceDoc);
      const stateArray = Array.from(encodedState);
      console.log('TauriSync: Sending snapshot:', {
        length: stateArray.length,
        deviceId: this.deviceId
      });

      // Add a delay before sending snapshot to ensure backend is ready
      await new Promise(resolve => setTimeout(resolve, 500));

      // Emit snapshot event
      await emit('sync-snapshot', stateArray);
      console.log('TauriSync: Successfully sent initial snapshot');

      // Confirm snapshot was sent
      await emit('snapshot-sent', { deviceId: this.deviceId });
    } catch (error) {
      console.error('TauriSync: Error sending initial snapshot:', error);
      // Notify about error
      await emit('sync-error', {
        error: error instanceof Error ? error.message : 'Unknown error',
        deviceId: this.deviceId
      });
    }
  }

  // Add method to force a snapshot resend
  public async resendSnapshot() {
    console.log('TauriSync: Forcing snapshot resend');
    await this.sendInitialSnapshot();
  }
}
