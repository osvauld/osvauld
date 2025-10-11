import * as Y from 'yjs';

/**
 * Simple Yjs document manager - no coordinator, no complex state management
 * Each builder creates its own instance
 */
export class SimpleYjsDoc {
  private doc: Y.Doc;
  private updateCallback?: (update: Uint8Array) => void;

  constructor() {
    this.doc = new Y.Doc();
  }

  /**
   * Get the Yjs document
   */
  getDoc(): Y.Doc {
    return this.doc;
  }

  /**
   * Load data from saved resource
   * @param data - Resource data with appropriate doc key (form_doc, blocksuite_doc, etc.)
   * @param docKey - Key to look for in data object
   */
  loadFromResource(data: any, docKey: string = 'form_doc'): void {
    if (!data) {
      console.warn(`⚠️ No data provided to load`);
      return;
    }

    try {
      // Parse if string
      const parsedData = typeof data === 'string' ? JSON.parse(data) : data;

      // Get updates from the specified doc key
      let updates: Uint8Array | null = null;

      if (parsedData[docKey]) {
        updates = Array.isArray(parsedData[docKey])
          ? new Uint8Array(parsedData[docKey])
          : null;
      }

      if (updates && updates.length > 0) {
        console.log(`📥 Loading ${updates.length} bytes from ${docKey}`);
        Y.applyUpdate(this.doc, updates);
        console.log(`✅ Loaded data from ${docKey}`);
      } else {
        console.log(`ℹ️ No existing data found in ${docKey}, starting fresh`);
      }
    } catch (error) {
      console.error(`❌ Error loading from ${docKey}:`, error);
    }
  }

  /**
   * Save current state
   * @param docKey - Key to use when saving (form_doc, blocksuite_doc, etc.)
   */
  saveToResource(docKey: string = 'form_doc'): any {
    const updates = Y.encodeStateAsUpdate(this.doc);

    return {
      [docKey]: Array.from(updates),
      last_modified: Date.now()
    };
  }

  /**
   * Set up auto-save on document changes
   */
  onUpdate(callback: (update: Uint8Array) => void): () => void {
    const handler = (update: Uint8Array, origin: any) => {
      // Don't trigger on loads (origin === 'loading')
      if (origin === 'loading') return;
      callback(update);
    };

    this.doc.on('update', handler);

    // Return cleanup function
    return () => {
      this.doc.off('update', handler);
    };
  }

  /**
   * Clean up
   */
  destroy(): void {
    this.doc.destroy();
  }
}
