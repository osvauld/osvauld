/**
 * Block Types for Loro CRDT Tree Structure
 *
 * Key Design:
 * - Block properties are stored in LoroTreeNode.data (LoroMap)
 * - Tree structure (id, parent, children) managed by Loro
 * - Editor layout (x, y, width, height) stored separately
 * - Fine-grained reactivity via stateKey/stateKeys
 */

/**
 * Block properties stored in Loro tree node.data
 *
 * Usage:
 *   node.data.set('type', 'container');
 *   node.data.set('name', 'My Container');
 *   const type = node.data.get('type');
 */
export interface Block {
  // ==========================================
  // Core (required)
  // ==========================================
  type: 'container' | 'text' | 'image' | 'input' | 'button' | 'collaborative';

  // ==========================================
  // Universal (all block types)
  // ==========================================

  /** Display name for tree view and UI */
  name?: string;

  /** Visibility: boolean or CEL expression string (e.g., "{{ user.isLoggedIn }}") */
  visible?: boolean | string;

  /** Custom CSS styling (supports CEL expressions) */
  css?: string;

  /** Order among siblings (for manual sorting) */
  order?: number;

  // ==========================================
  // Fine-Grained Reactivity
  // ==========================================

  /** Primary state dependency path (e.g., "user.email") */
  stateKey?: string;

  /** Multiple state dependency paths (for complex CEL expressions) */
  stateKeys?: string[];

  // ==========================================
  // Loop Support (forEach)
  // ==========================================

  /** Array name in templateState to loop over */
  forEach?: string;

  /** Variable name for loop item (default: 'item') */
  forEachAs?: string;

  // ==========================================
  // Type-Specific Properties
  // ==========================================

  // ----- container -----
  /** Mark as entry point screen (for navigation) */
  isEntryPoint?: boolean;

  /** Render as modal overlay */
  isModal?: boolean;

  // ----- text -----
  /** Text content (supports CEL expressions) */
  content?: string;

  /** Rendering mode for text */
  mode?: 'plain' | 'markdown' | 'html';

  /** Heading level (1-6) for semantic HTML */
  level?: 1 | 2 | 3 | 4 | 5 | 6;

  // ----- image -----
  /** Image URL or path (supports CEL expressions) */
  src?: string;

  /** Alt text for accessibility */
  alt?: string;

  // ----- input -----
  /** Input field type */
  inputType?: 'text' | 'email' | 'number' | 'textarea' | 'checkbox' | 'select' | 'radio';

  /** Parent form ID (groups related inputs) */
  formId?: string;

  /** Field identifier for form submission */
  fieldName?: string;

  /** Field label text */
  label?: string;

  /** Placeholder text for input */
  placeholder?: string;

  /** Required field validation */
  required?: boolean;

  /** Default value for input */
  defaultValue?: any;

  /** Options for select/radio inputs */
  options?: Array<{ label: string; value: any }>;

  // ----- button -----
  /** Button action type */
  action?: 'navigate' | 'setState' | 'submit';

  /** Target container ID for navigation */
  targetContainerId?: string;

  /** Single state value to set (supports CEL) */
  stateValue?: any;

  /** Multiple state values to set (supports CEL in values) */
  stateUpdates?: Record<string, any>;

  /** Mark as form submit button */
  submit?: boolean;

  /** Field name for caching value (form mode 3A) */
  fieldName?: string;

  /** Value to cache (supports CEL) */
  value?: any;

  /** Event name for form submissions */
  eventName?: string;

  // ----- collaborative -----
  /** Thread/discussion title */
  title?: string;

  /** Thread/discussion description */
  description?: string;
}

/**
 * Block with runtime metadata (used in components)
 */
export interface BlockWithMeta extends Block {
  /** Node ID from Loro tree */
  id: string;

  /** Whether this block has children */
  hasChildren?: boolean;

  /** Number of child blocks */
  childCount?: number;

  /** Parent block ID (for breadcrumbs, etc.) */
  parentId?: string | null;
}

/**
 * Editor layout data (separate from block content)
 * Stored in a separate map or locally in editor
 */
export interface EditorLayout {
  /** Block/node ID this layout applies to */
  blockId: string;

  /** X position in canvas */
  x: number;

  /** Y position in canvas */
  y: number;

  /** Width in canvas */
  width: number;

  /** Height in canvas */
  height: number;

  /** Z-index for stacking */
  zIndex?: number;
}

/**
 * Viewport state for canvas
 */
export interface Viewport {
  x: number;
  y: number;
  zoom: number;
}

/**
 * User information for collaboration
 */
export interface UserInfo {
  name: string;
  color: string;
  id: number;
  userId: string;
}

/**
 * Collaborator presence
 */
export interface Collaborator {
  id: string;
  name: string;
  color: string;
  clientId: number;
}

/**
 * User details (authentication)
 */
export interface UserDetails {
  userId: string;
  deviceId: string;
  username: string;
  publicKey: string;
  deviceKey: string;
}

/**
 * Comment/collaborative item
 */
export interface CollaborativeItem {
  id: string;
  author: string;
  userId?: string;
  timestamp: number;
  content?: string;
  [key: string]: any;  // Allow custom fields
}

/**
 * Legacy types (for backward compatibility during migration)
 */

/** @deprecated Use Block interface instead */
export interface FormField {
  id: string;
  type: "text" | "email" | "number" | "textarea" | "checkbox";
  label: string;
  placeholder?: string;
  required: boolean;
}

/** @deprecated Use Block interface instead */
export interface FormConfig {
  fields: FormField[];
  submitButtonText: string;
}

/** @deprecated Use CollaborativeItem instead */
export interface NoticeMessage {
  id: string;
  username: string;
  message: string;
  timestamp: number;
  userId: string;
}

/**
 * Type guards
 */

export function isContainer(block: Block): boolean {
  return block.type === 'container';
}

export function isText(block: Block): boolean {
  return block.type === 'text';
}

export function isImage(block: Block): boolean {
  return block.type === 'image';
}

export function isInput(block: Block): boolean {
  return block.type === 'input';
}

export function isButton(block: Block): boolean {
  return block.type === 'button';
}

export function isCollaborative(block: Block): boolean {
  return block.type === 'collaborative';
}

/**
 * Helper functions
 */

/** Check if block can have children */
export function canHaveChildren(block: Block): boolean {
  return block.type === 'container';
}

/** Get display name for block */
export function getBlockDisplayName(block: Block): string {
  if (block.name) return block.name;

  switch (block.type) {
    case 'container':
      return block.isEntryPoint ? 'Entry Screen' : 'Container';
    case 'text':
      return block.level ? `Heading ${block.level}` : 'Text';
    case 'image':
      return 'Image';
    case 'input':
      return block.label || `Input (${block.inputType || 'text'})`;
    case 'button':
      return block.content as string || 'Button';
    case 'collaborative':
      return block.title || 'Discussion';
    default:
      return 'Block';
  }
}
