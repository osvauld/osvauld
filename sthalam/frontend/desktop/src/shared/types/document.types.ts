/**
 * Document Types - Loro CRDT documents
 */

import type { LoroDoc } from 'loro-crdt';

/**
 * The 6 core Loro documents that make up Sthalam's state system
 */
export interface Documents {
  templateDoc: LoroDoc;       // Template structure (Tree)
  uiStateDoc: LoroDoc;        // UI state (Tree) - for component state
  contentDoc: LoroDoc;        // Publisher content (Map)
  userContentDoc: LoroDoc;    // User state (Map)
  collaborativeDoc: LoroDoc;  // Comments/threads (Map of Trees)
  submissionsDoc: LoroDoc;    // Form submissions (Map of Lists)
}
