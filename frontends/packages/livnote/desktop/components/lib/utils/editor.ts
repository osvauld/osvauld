import * as Y from 'yjs';
import { Awareness } from 'y-protocols/awareness';
import { sendMessage } from "@osvauld/password-manager-common/utils/helper.ts";
import { emit } from "@tauri-apps/api/event";

export class Editor {
  private ydoc: Y.Doc;
  private type: Y.XmlFragment;
  private awareness: Awareness;
  private clientID: number;

  constructor() {
    this.clientID = Math.floor(Math.random() * 0xffffffff);
    this.initYjs();
  }

  private initYjs() {
    this.ydoc = new Y.Doc();
    this.type = this.ydoc.getXmlFragment('prosemirror');
    this.awareness = new Awareness(this.ydoc);

    this.awareness.setLocalState({
      user: {
        name: `User ${this.clientID}`,
        color: `#${Math.floor(Math.random() * 16777215).toString(16)}`,
      },
    });
  }

  getYjsDoc() {
    return {
      ydoc: this.ydoc,
      type: this.type,
      awareness: this.awareness,
      clientID: this.clientID
    };
  }

  async handleCollaborationUpdate(update: any) {
    try {
      await emit("sync-update", JSON.stringify({
        update: Array.from(update),
        clientID: this.clientID
      }));
    } catch (error) {
      console.error("Error handling collaboration update:", error);
      throw error;
    }
  }

  applyUpdate(update: Uint8Array | number[], sender: number) {
    if (sender === this.clientID) return;

    try {
      const updateArray = update instanceof Uint8Array ? update : new Uint8Array(update);
      Y.applyUpdate(this.ydoc, updateArray);
    } catch (error) {
      console.error("Error applying update:", error);
      throw error;
    }
  }

  destroy() {
    if (this.ydoc) {
      this.ydoc.destroy();
    }
  }
}

// Export a singleton instance
export const editorInstance = new Editor();
