import type * as Y from 'yjs';

class EditorStore {
  doc = $state<Y.Doc | null>(null);

  setDoc(doc: Y.Doc) {
    console.log('🪙 [Store] setDoc called');
    this.doc = doc;
  }

  clear() {
    console.log('🗑️ [Store] clear called');
    if (this.doc) {
      const yBlocks = this.doc.getArray('blocks');
      yBlocks.delete(0, yBlocks.length);
      console.log('✅ Document cleared');
    }
  }
}

export const editorStore = new EditorStore();
