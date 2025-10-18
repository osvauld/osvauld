import * as Y from "yjs";

/**
 * Manages blocksuite document blocks using YJS
 * Follows livnote's CommentsStore pattern
 */
export class BlocksuiteStore {
  private blocks: Y.Map<any> | null = null;
  private observers: Set<() => void> = new Set();

  /**
   * Set the YJS map for blocks
   */
  setBlocksMap(blocks: Y.Map<any>): void {
    // Clean up old observer if any
    if (this.blocks) {
      this.blocks.unobserveDeep(this.handleBlocksChange);
    }

    this.blocks = blocks;

    // Set up observer
    this.blocks.observeDeep(this.handleBlocksChange);

    console.log("📦 [BlocksuiteStore] Yjs map set and observer attached");
  }

  /**
   * Handle changes to blocks
   */
  private handleBlocksChange = (): void => {
    console.log("📦 [BlocksuiteStore] Yjs change detected, notifying observers");
    this.notifyObservers();
  };

  /**
   * Get all blocks
   */
  getAllBlocks(): Map<string, any> {
    if (!this.blocks) {
      return new Map();
    }

    const blocksMap = new Map<string, any>();
    this.blocks.forEach((value, key) => {
      blocksMap.set(key, value);
    });

    return blocksMap;
  }

  /**
   * Get a specific block
   */
  getBlock(blockId: string): any | null {
    if (!this.blocks) {
      return null;
    }

    return this.blocks.get(blockId) || null;
  }

  /**
   * Subscribe to block changes
   */
  subscribe(callback: () => void): () => void {
    this.observers.add(callback);
    return () => {
      this.observers.delete(callback);
    };
  }

  /**
   * Notify all observers
   */
  private notifyObservers(): void {
    this.observers.forEach((callback) => callback());
  }

  /**
   * Clean up
   */
  destroy(): void {
    if (this.blocks) {
      this.blocks.unobserveDeep(this.handleBlocksChange);
    }

    this.observers.clear();
    this.blocks = null;
  }
}
