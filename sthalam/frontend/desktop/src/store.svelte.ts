import type * as Y from 'yjs';

export interface UserDetails {
  userId: string;
  deviceId: string;
  username: string;
  publicKey: string;
  deviceKey: string;
}

class EditorStore {
  doc = $state<Y.Doc | null>(null);
  userDetails = $state<UserDetails | null>(null);

  setDoc(doc: Y.Doc) {
    console.log('🪙 [Store] setDoc called');
    this.doc = doc;
  }

  setUserDetails(details: UserDetails) {
    console.log('👤 [Store] setUserDetails called', details.username);
    this.userDetails = details;
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
