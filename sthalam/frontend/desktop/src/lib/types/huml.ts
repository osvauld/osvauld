/**
 * Complete HUML Type Definitions
 *
 * Based on STHALAM_DSL.md specification
 */

/**
 * Base block interface - all blocks extend this
 */
export interface BaseBlock {
  /** Conditional visibility */
  when?: string;  // CEL expression

  /** Custom CSS styles */
  css?: string;

  /** Custom HTML classes */
  class?: string;

  /** Iteration - render block for each item */
  forEach?: string;  // CEL expression returning array
  as?: string;       // Variable name for loop item
  key?: string;      // Property name for keyed iteration

  /** Pagination */
  limit?: number;
  offset?: number;
}

/**
 * Layout Blocks
 */

export interface ScreenBlock extends BaseBlock {
  type: 'screen';
  name: string;
  blocks?: Block[];
}

export interface ContainerBlock extends BaseBlock {
  type: 'container';
  layout?: 'flex' | 'grid' | 'block';
  direction?: 'row' | 'column';
  gap?: string;
  alignItems?: 'start' | 'center' | 'end' | 'stretch';
  justifyContent?: 'start' | 'center' | 'end' | 'space-between' | 'space-around' | 'space-evenly';
  columns?: number;  // For grid layout
  blocks?: Block[];
}

export interface SectionBlock extends BaseBlock {
  type: 'section';
  blocks?: Block[];
}

/**
 * Content Blocks
 */

export interface TextBlock extends BaseBlock {
  type: 'text';
  content: string;  // Can contain {{ }} interpolations
}

export interface HeadingBlock extends BaseBlock {
  type: 'heading';
  level?: 1 | 2 | 3 | 4 | 5 | 6;
  content: string;
}

export interface LabelBlock extends BaseBlock {
  type: 'label';
  for?: string;  // Input ID
  content: string;
}

export interface ImageBlock extends BaseBlock {
  type: 'image';
  src: string;
  alt?: string;
  width?: string | number;
  height?: string | number;
}

export interface VideoBlock extends BaseBlock {
  type: 'video';
  src: string;
  controls?: boolean;
  autoplay?: boolean;
  loop?: boolean;
  muted?: boolean;
  width?: string | number;
  height?: string | number;
}

/**
 * Input Blocks
 */

export interface InputBlock extends BaseBlock {
  type: 'input';
  name: string;
  value?: string;  // CEL expression
  placeholder?: string;
  inputType?: 'text' | 'email' | 'password' | 'number' | 'tel' | 'url' | 'search';
  disabled?: string;  // CEL expression
  required?: boolean;

  /** Validation */
  validate?: string;  // CEL expression returning boolean
  error?: string;     // Error message to show

  /** Events */
  onChange?: string;  // Action name
  onBlur?: string;
  onFocus?: string;
}

export interface TextareaBlock extends BaseBlock {
  type: 'textarea';
  name: string;
  value?: string;
  placeholder?: string;
  rows?: number;
  cols?: number;
  disabled?: string;
  required?: boolean;
  validate?: string;
  error?: string;
  onChange?: string;
  onBlur?: string;
  onFocus?: string;
}

export interface CheckboxBlock extends BaseBlock {
  type: 'checkbox';
  name: string;
  checked?: string;  // CEL expression
  label?: string;
  disabled?: string;
  onChange?: string;
}

export interface SelectBlock extends BaseBlock {
  type: 'select';
  name: string;
  value?: string;
  options: Array<{
    value: string;
    label: string;
  }>;
  disabled?: string;
  required?: boolean;
  onChange?: string;
}

export interface RadioBlock extends BaseBlock {
  type: 'radio';
  name: string;
  value?: string;  // CEL expression for selected value
  options: Array<{
    value: string;
    label: string;
  }>;
  disabled?: string;
  onChange?: string;
}

/**
 * Action Blocks
 */

export interface ButtonBlock extends BaseBlock {
  type: 'button';
  content: string;
  action?: string;  // Action name to dispatch
  params?: Record<string, any>;  // Action parameters
  disabled?: string;  // CEL expression
  submitForm?: boolean;  // If true, type="submit"
  onClick?: string;  // Action name
}

export interface LinkBlock extends BaseBlock {
  type: 'link';
  content: string;
  href?: string;  // Regular link

  /** OR use action for SPA navigation */
  action?: 'navigate';
  params?: {
    screen?: string;
    [key: string]: any;
  };
}

export interface FormBlock extends BaseBlock {
  type: 'form';
  name: string;
  blocks?: Block[];
  onSubmit?: string;  // Action name
}

/**
 * Special Blocks
 */

export interface CanvasBlock extends BaseBlock {
  type: 'canvas';
  mode?: 'pattern' | 'chart' | 'interactive' | 'custom';
  width?: number;
  height?: number;

  /** Pattern mode (current implementation) */
  gridSize?: number;
  cellSize?: number;
  pattern?: string;  // CEL expression for selected pattern
  expressions?: Record<string, string>;  // Pattern name → CEL expression

  /** Animation */
  autoplay?: boolean;
  fps?: number;
  onRender?: string;  // Action called after each frame
}

export interface ModalBlock extends BaseBlock {
  type: 'modal';
  name: string;
  visible?: string;  // CEL expression
  size?: 'small' | 'medium' | 'large' | 'fullscreen';
  closable?: boolean;  // Show close button (default: true)
  blocks?: Block[];
}

/**
 * Control Flow Blocks
 */

export interface IfBlock extends BaseBlock {
  if: string;  // CEL expression
  then?: Block[];
  else?: Block[];
}

export interface MatchBlock extends BaseBlock {
  match: string;  // CEL expression
  cases: Array<{
    value?: string;  // Match value
    default?: boolean;  // Default case
    blocks?: Block[];
  }>;
}

/**
 * Union of all block types
 */
export type Block =
  | ScreenBlock
  | ContainerBlock
  | SectionBlock
  | TextBlock
  | HeadingBlock
  | LabelBlock
  | ImageBlock
  | VideoBlock
  | InputBlock
  | TextareaBlock
  | CheckboxBlock
  | SelectBlock
  | RadioBlock
  | ButtonBlock
  | LinkBlock
  | FormBlock
  | CanvasBlock
  | ModalBlock
  | IfBlock
  | MatchBlock;

/**
 * Evaluation context
 * Contains all variables available to CEL expressions
 */
export interface Context {
  [key: string]: any;
}

/**
 * Action handler function
 */
export type ActionHandler = (action: string, params?: any) => void;

/**
 * State change handler function
 */
export type StateChangeHandler = (key: string, value: any) => void;

/**
 * Block renderer props
 */
export interface BlockRendererProps {
  block: Block;
  context: Context;
  onAction?: ActionHandler;
  onStateChange?: StateChangeHandler;
}

/**
 * HUML Template structure
 */
export interface HUMLTemplate {
  name: string;
  version?: string;

  /** Document schemas (state structure) */
  documents?: Record<string, DocumentSchema>;

  /** Computed values */
  computed?: Record<string, ComputedValue>;

  /** UI definition */
  ui?: {
    viewer?: Block[];
    publisher?: Block[];
  };
}

/**
 * Document field schema
 */
export interface DocumentFieldSchema {
  type: 'string' | 'number' | 'boolean' | 'list' | 'map' | 'text' | 'tree';
  initial?: any;
  fields?: Record<string, DocumentFieldSchema>;  // For nested objects
}

/**
 * Document schema
 */
export type DocumentSchema = Record<string, DocumentFieldSchema>;

/**
 * Computed value definition
 */
export interface ComputedValue {
  type: 'string' | 'number' | 'boolean' | 'list' | 'map';
  expr: string;  // CEL expression
  depends?: string[];  // Dependency paths (e.g., ["appState.counter"])
}

/**
 * Navigation state
 */
export interface NavigationState {
  currentScreen: string;
  params: Record<string, any>;
  history: Array<{
    screen: string;
    params: Record<string, any>;
  }>;
}

/**
 * Modal state
 */
export interface ModalState {
  [modalName: string]: {
    visible: boolean;
    data?: any;
  };
}

/**
 * Action types (built-in)
 */
export type BuiltInAction = 'navigate' | 'openModal' | 'closeModal' | 'setState';

/**
 * Action parameters for built-in actions
 */
export interface NavigateParams {
  screen: string;
  [key: string]: any;
}

export interface OpenModalParams {
  modal: string;
  data?: any;
}

export interface CloseModalParams {
  modal: string;
}

export interface SetStateParams {
  field: string;
  value: any;
}
