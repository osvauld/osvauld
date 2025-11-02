/**
 * Block Types for Loro-based Templates
 *
 * IMPORTANT: Text content is stored as LoroText, not plain strings!
 * Access via: blockDataMap.getText('content')
 *
 * This enables efficient incremental sync - only deltas sent over network
 */

import type { TreeID } from 'loro-crdt';

// ============================================
// BASE INTERFACES (Shared Properties)
// ============================================

/**
 * Base properties that ALL blocks have
 */
export interface BaseBlock {
  id: string;              // Human-readable ID (for HUML references like targetContainerId)
  treeId?: TreeID;         // Loro TreeID (for event tracking and tree operations)
  type: string;            // Block type discriminator
  name?: string;           // Block name/label
  css?: string;            // CSS styles (may contain CEL expressions)
  visible?: boolean | string; // Visibility (boolean or CEL expression)
  uiStateRef?: string;     // Reference to uiStateTree node path (e.g., "screens.home.navbar")
}

/**
 * Blocks with text content
 * NOTE: Content is stored as LoroText in the tree, not as a plain string!
 */
export interface WithContent {
  // content is LoroText - access via blockDataMap.getText('content')
  // When rendering: contentText.toString() to get string
}

/**
 * Blocks with navigation/state actions
 */
export interface WithActions {
  action?: 'setState';
  target?: 'state' | 'content' | 'user_content' | 'ui'; // Which Loro document to update
  stateKey?: string;       // Single state key to update
  stateValue?: any;        // Single state value (may contain CEL expression)
  stateUpdates?: Record<string, any>; // Bulk state updates (values may contain CEL)
  targetContainerId?: string; // Navigation target (screen/container ID)
}

/**
 * Form-related blocks
 */
export interface WithForm {
  formId?: string;         // Form ID this field belongs to
  fieldName?: string;      // Field name for submission
  submit?: boolean;        // Is this a submit button?
  required?: boolean;      // Is field required?
  label?: string;          // Field label
  placeholder?: string;    // Field placeholder
  stateKey?: string;       // For real-time state binding (reactive forms)
}

/**
 * Loop rendering (forEach)
 */
export interface WithLoop {
  forEach?: string;        // Array name from state to loop over
  forEachAs?: string;      // Custom loop variable name (default: 'item')
}

// ============================================
// CONTAINER BLOCKS
// ============================================

/**
 * Screen Container - Top-level screen/page
 */
export interface ScreenContainerBlock extends BaseBlock {
  type: 'screen-container';
  isEntryPoint?: boolean;  // Is this the initial screen?
}

/**
 * Section Container - Generic container for grouping blocks
 */
export interface SectionContainerBlock extends BaseBlock, WithLoop {
  type: 'section-container';
  name: string;            // REQUIRED for section-container per HUML spec
}

/**
 * Modal Container - Popup/overlay container
 */
export interface ModalBlock extends BaseBlock {
  type: 'modal';
  name: string;
}

// ============================================
// TEXT CONTENT BLOCKS
// ============================================

/**
 * Heading Block
 * Content stored as LoroText
 */
export interface HeadingBlock extends BaseBlock, WithContent {
  type: 'heading';
}

/**
 * Text Block
 * Content stored as LoroText
 */
export interface TextBlock extends BaseBlock, WithContent {
  type: 'text';
}

/**
 * Markdown Text Block
 * Content stored as LoroText, rendered via markdown parser
 *
 * Rendering: marked(contentLoroText.toString())
 */
export interface MarkdownTextBlock extends BaseBlock, WithContent {
  type: 'markdown-text';
  mode?: 'markdown' | 'html';
}

// ============================================
// MEDIA BLOCKS
// ============================================

/**
 * Image Block
 */
export interface ImageBlock extends BaseBlock {
  type: 'image';
  src: string;             // Image URL (may contain CEL)
  alt?: string;            // Alt text
}

// ============================================
// INTERACTIVE BLOCKS
// ============================================

/**
 * Navigation Button
 * Can be used for: navigation, setState, form submission, choice buttons
 */
export interface NavButtonBlock extends BaseBlock, WithContent, WithActions, WithForm {
  type: 'nav-button';
  // content: LoroText - button text
  fieldName?: string;      // For choice buttons (sets this field to 'value')
  value?: any;             // Value to set when clicked (choice buttons)
}

// ============================================
// FORM BLOCKS
// ============================================

/**
 * Form Metadata Block
 * Defines form configuration
 */
export interface FormBlock extends BaseBlock {
  type: 'form';
  name: string;            // Form name (REQUIRED)
  eventName?: string;      // Event name for tracking (e.g., "contact_submission")
  submitEndpoint?: string; // URL to submit form data (e.g., Formspree)
}

/**
 * Form Field - Text Input
 */
export interface FormFieldTextBlock extends BaseBlock, WithForm {
  type: 'form-field-text';
  name: string;            // Field name (REQUIRED)
}

/**
 * Form Field - Email Input
 */
export interface FormFieldEmailBlock extends BaseBlock, WithForm {
  type: 'form-field-email';
  name: string;
}

/**
 * Form Field - Telephone Input
 */
export interface FormFieldTelBlock extends BaseBlock, WithForm {
  type: 'form-field-tel';
  name: string;
}

/**
 * Form Field - Password Input
 */
export interface FormFieldPasswordBlock extends BaseBlock, WithForm {
  type: 'form-field-password';
  name: string;
}

/**
 * Form Field - Number Input
 */
export interface FormFieldNumberBlock extends BaseBlock, WithForm {
  type: 'form-field-number';
  name: string;
}

/**
 * Form Field - Textarea
 */
export interface FormFieldTextareaBlock extends BaseBlock, WithForm {
  type: 'form-field-textarea';
  name: string;
}

/**
 * Form Field - Checkbox
 */
export interface FormFieldCheckboxBlock extends BaseBlock, WithForm {
  type: 'form-field-checkbox';
  name: string;
}

/**
 * Form Field - Select Dropdown
 */
export interface FormFieldSelectBlock extends BaseBlock, WithForm {
  type: 'form-field-select';
  name: string;
  options?: string[];      // Select options
}

// ============================================
// COLLABORATIVE BLOCKS
// ============================================

/**
 * Thread/Comments Block
 * Each comment uses LoroText for collaborative rich text editing
 * Later: Can add ProseMirror for WYSIWYG editing
 */
export interface ThreadBlock extends BaseBlock {
  type: 'thread';
  mode?: 'markdown' | 'plain';
  description?: string;    // Placeholder text
  // Comments stored in collaborativeDoc, each with its own LoroText
}

// ============================================
// DISCRIMINATED UNION
// ============================================

/**
 * All possible block types
 * TypeScript discriminated union for type-safe block handling
 */
export type Block =
  // Containers
  | ScreenContainerBlock
  | SectionContainerBlock
  | ModalBlock
  // Text content
  | HeadingBlock
  | TextBlock
  | MarkdownTextBlock
  // Media
  | ImageBlock
  // Interactive
  | NavButtonBlock
  // Forms
  | FormBlock
  | FormFieldTextBlock
  | FormFieldEmailBlock
  | FormFieldTelBlock
  | FormFieldPasswordBlock
  | FormFieldNumberBlock
  | FormFieldTextareaBlock
  | FormFieldCheckboxBlock
  | FormFieldSelectBlock
  // Collaborative
  | ThreadBlock;

/**
 * Block type string literal union
 */
export type BlockType = Block['type'];

// ============================================
// TYPE GUARDS
// ============================================

export function isScreenContainer(block: Block): block is ScreenContainerBlock {
  return block.type === 'screen-container';
}

export function isSectionContainer(block: Block): block is SectionContainerBlock {
  return block.type === 'section-container';
}

export function isContainer(block: Block): block is ScreenContainerBlock | SectionContainerBlock | ModalBlock {
  return block.type === 'screen-container'
    || block.type === 'section-container'
    || block.type === 'modal';
}

export function isFormField(block: Block): block is
  | FormFieldTextBlock
  | FormFieldEmailBlock
  | FormFieldTelBlock
  | FormFieldPasswordBlock
  | FormFieldNumberBlock
  | FormFieldTextareaBlock
  | FormFieldCheckboxBlock
  | FormFieldSelectBlock {
  return block.type.startsWith('form-field-');
}

export function hasContent(block: Block): block is HeadingBlock | TextBlock | MarkdownTextBlock | NavButtonBlock {
  return block.type === 'heading'
    || block.type === 'text'
    || block.type === 'markdown-text'
    || block.type === 'nav-button';
}

export function hasActions(block: Block): block is NavButtonBlock {
  return block.type === 'nav-button';
}

export function isTextBlock(block: Block): block is HeadingBlock | TextBlock | MarkdownTextBlock {
  return block.type === 'heading' || block.type === 'text' || block.type === 'markdown-text';
}