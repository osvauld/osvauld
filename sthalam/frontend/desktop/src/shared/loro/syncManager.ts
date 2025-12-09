/**
 * SyncManager - Real-time auto-sync between frontend Loro and backend Scribe
 *
 * Subscribes to Loro document changes and sends incremental updates to backend.
 * Prevents echo loops by tracking when remote updates are being applied.
 */

import { LoroDoc } from 'loro-crdt';
import { applyUpdate } from '../../utils/scribe';
import type { Documents } from './loroCoordinator';

export class SyncManager {
  private pageId: string | null = null;
  private isApplyingRemote = false;
  private lastSentVersions: Map<string, Uint8Array> = new Map();
  private unsubscribes: Map<string, () => void> = new Map();

  /**
   * Start syncing for a page
   * Subscribes to all Loro documents and sends changes to backend
   */
  startSync(pageId: string, documents: Documents) {
    this.stopSync(); // Clean up previous
    this.pageId = pageId;
    this.lastSentVersions.clear();

    console.log('[SyncManager] Starting sync for page:', pageId);

    // Documents to sync (excluding uiStateDoc which is session-only)
    const docsToSync: [string, LoroDoc][] = [
      ['templateDoc', documents.templateDoc],
      ['contentDoc', documents.contentDoc],
      ['userContentDoc', documents.userContentDoc],
      ['collaborativeDoc', documents.collaborativeDoc],
      ['submissionsDoc', documents.submissionsDoc],
    ];

    // Subscribe to each document
    for (const [docName, doc] of docsToSync) {
      const layerName = this.docNameToLayerName(docName);

      // Store initial version vector
      const versionBytes = doc.oplogVersion().encode();
      this.lastSentVersions.set(layerName, versionBytes);

      // Subscribe to changes
      const unsub = doc.subscribe(() => {
        if (this.isApplyingRemote) {
          console.log('[SyncManager] Skipping send - applying remote update');
          return;
        }
        this.sendUpdate(layerName, doc);
      });

      this.unsubscribes.set(docName, unsub);
      console.log(`[SyncManager] Subscribed to ${docName} → ${layerName}`);
    }
  }

  /**
   * Stop syncing (when switching pages or closing)
   */
  stopSync() {
    if (this.pageId) {
      console.log('[SyncManager] Stopping sync for page:', this.pageId);
    }

    for (const unsub of this.unsubscribes.values()) {
      unsub();
    }
    this.unsubscribes.clear();
    this.lastSentVersions.clear();
    this.pageId = null;
  }

  /**
   * Apply update from backend (P2P peer)
   * Sets flag to prevent echo - the subscribe callback will see isApplyingRemote=true
   */
  applyRemoteUpdate(layerName: string, update: Uint8Array, doc: LoroDoc) {
    console.log(`[SyncManager] Applying remote update for ${layerName}, ${update.length} bytes`);

    this.isApplyingRemote = true;
    try {
      doc.import(update);
      // Update our version to include remote changes
      this.lastSentVersions.set(layerName, doc.oplogVersion().encode());
    } finally {
      this.isApplyingRemote = false;
    }
  }

  /**
   * Send incremental update to backend
   */
  private async sendUpdate(layerName: string, doc: LoroDoc) {
    if (!this.pageId) return;

    const lastVersionBytes = this.lastSentVersions.get(layerName);

    // Export delta since last sent version
    let update: Uint8Array;
    if (lastVersionBytes) {
      // Decode the stored version vector and export updates since then
      const { VersionVector } = await import('loro-crdt');
      const lastVersion = VersionVector.decode(lastVersionBytes);
      update = doc.export({ mode: "update", from: lastVersion });
    } else {
      // No previous version - export full update
      update = doc.export({ mode: "update" });
    }

    if (update.length === 0) {
      console.log(`[SyncManager] No changes for ${layerName}, skipping`);
      return;
    }

    console.log(`[SyncManager] Sending ${update.length} bytes for ${layerName}`);

    try {
      await applyUpdate(this.pageId, layerName, update);
      // Update last sent version after successful send
      this.lastSentVersions.set(layerName, doc.oplogVersion().encode());
      console.log(`[SyncManager] Successfully sent update for ${layerName}`);
    } catch (error) {
      console.error('[SyncManager] Failed to send update:', error);
      // Don't update lastSentVersions - will retry on next change
    }
  }

  /**
   * Map frontend document names to backend layer names
   */
  private docNameToLayerName(docName: string): string {
    const mapping: Record<string, string> = {
      'templateDoc': 'template_doc',
      'contentDoc': 'content_doc',
      'userContentDoc': 'user_content_doc',
      'collaborativeDoc': 'collaborative_doc',
      'submissionsDoc': 'submissions_doc',
    };
    return mapping[docName] || docName;
  }

  /**
   * Map backend layer names to frontend document names
   */
  layerNameToDocName(layerName: string): string {
    const mapping: Record<string, string> = {
      'template_doc': 'templateDoc',
      'content_doc': 'contentDoc',
      'user_content_doc': 'userContentDoc',
      'collaborative_doc': 'collaborativeDoc',
      'submissions_doc': 'submissionsDoc',
    };
    return mapping[layerName] || layerName;
  }

  /**
   * Check if currently syncing a page
   */
  isActive(): boolean {
    return this.pageId !== null;
  }

  /**
   * Get current page ID being synced
   */
  getCurrentPageId(): string | null {
    return this.pageId;
  }
}

// Singleton instance
export const syncManager = new SyncManager();
