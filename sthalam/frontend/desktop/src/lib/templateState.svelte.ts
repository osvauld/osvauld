/**
 * Template State Manager - Svelte 5 Runes (Native Signals)
 *
 * Manages global state for HUML templates with JEXL expression support.
 * Uses Svelte's native $state rune for automatic fine-grained reactivity.
 * Blocks only re-render when properties they ACCESS actually change.
 *
 * Supports multi-document architecture:
 * - content (publisher-owned, from content_doc)
 * - user_content (per-user state, from user_content_doc)
 * - state (backward compatibility, in-memory)
 * - computed (derived/reactive computations)
 */

import type * as Y from 'yjs';
import { evaluateExpression, extractExpression } from '../utils/jexlEvaluator';

export interface TemplateState {
  [key: string]: any;
}

interface ComputedDefinition {
  expression: string;
  dependencies: string[];
}

class TemplateStateManager {
  // Reactive state using Svelte 5 runes
  // Each property is automatically tracked by Svelte
  state = $state<Record<string, any>>({});

  // Computed properties (reactive/derived)
  private computedDefs: Map<string, ComputedDefinition> = new Map();
  private computedValues = $state<Record<string, any>>({});

  // Change counter to trigger manual updates when needed
  private version = $state(0);

  // Store references to Yjs maps for observing changes
  private contentMap: Y.Map<any> | null = null;
  private userContentMap: Y.Map<any> | null = null;
  private observeHandlers: Array<() => void> = [];

  // Flag to prevent recomputation during bulk initialization
  private isInitializing: boolean = false;

  /**
   * Initialize state from template definition (HUML template's state:: section)
   * Also persists to Yjs documents if they're available
   */
  initialize(stateDefinition?: Record<string, any>) {
    // Set flag to prevent premature recomputation
    this.isInitializing = true;

    // Clear existing state
    this.state = {};

    // Only initialize if state definition is provided from template
    if (stateDefinition) {
      // Set each property in the reactive state
      for (const [key, value] of Object.entries(stateDefinition)) {
        this.state[key] = value;

        // Also persist to content Yjs document if available
        // This ensures the state persists when documents are reloaded
        if (this.contentMap) {
          console.log(`💾 [TemplateState] Writing to contentMap: ${key} =`, value);
          this.contentMap.set(key, value);
        }
      }

      // Increment version
      this.version++;

      console.log(`📊 [TemplateState] Initialized with Svelte runes:`, Object.keys(stateDefinition));
      console.log(`📊 [TemplateState] State persisted to content doc:`, this.contentMap !== null);
      console.log(`📊 [TemplateState] ContentMap size after init:`, this.contentMap?.size);
    } else {
      console.log(`📊 [TemplateState] No state definition provided, starting with empty state`);
    }

    // Done initializing - now safe to recompute
    this.isInitializing = false;

    // Recompute all computed properties now that state is fully loaded
    if (this.computedDefs.size > 0) {
      this.recomputeAll();
    }
  }

  /**
   * Set a single state property
   * Supports nested keys like "user.name"
   * Also persists to userContent Yjs document if available
   */
  set(key: string, value: any) {
    const keys = key.split('.');

    if (keys.length === 1) {
      // Simple key - set directly in reactive state
      this.state[key] = value;

      // Persist to userContent Yjs document (for per-user mutable state)
      if (this.userContentMap) {
        this.userContentMap.set(key, value);
        console.log(`💾 [TemplateState] Persisted ${key} to userContent doc`);
      }
    } else {
      // Nested key - get root object, deep clone, update, set back
      const rootKey = keys[0];
      let rootValue = this.getValue(rootKey) || {};

      // Deep clone
      const newValue = JSON.parse(JSON.stringify(rootValue));
      let current: any = newValue;

      for (let i = 1; i < keys.length - 1; i++) {
        const k = keys[i];
        if (!current[k] || typeof current[k] !== 'object') {
          current[k] = {};
        }
        current = current[k];
      }

      current[keys[keys.length - 1]] = value;

      // Update the root state property
      this.state[rootKey] = newValue;

      // Persist to Yjs
      if (this.userContentMap) {
        this.userContentMap.set(rootKey, newValue);
        console.log(`💾 [TemplateState] Persisted ${rootKey} to userContent doc (nested update)`);
      }
    }

    // Increment version to trigger reactivity
    this.version++;

    // Recompute computed properties if any are defined
    if (this.computedDefs.size > 0) {
      this.recomputeAll();
    }

    console.log(`📊 [TemplateState] Set ${key} = ${JSON.stringify(value)}`);
    console.log(`📊 [TemplateState] Current state:`, this.get());
  }

  /**
   * Update multiple state properties at once
   */
  update(updates: Record<string, any>) {
    for (const [key, value] of Object.entries(updates)) {
      this.set(key, value);
    }
    console.log(`📊 [TemplateState] Bulk update:`, updates);
  }

  /**
   * Get current state object (includes base state + computed properties)
   * When accessed inside a Svelte component, automatically tracks dependencies
   * Svelte's reactivity system handles the fine-grained tracking
   */
  get(): TemplateState {
    // Return merged state (base + computed)
    // Svelte automatically tracks which properties are accessed
    return { ...this.state, ...this.computedValues };
  }

  /**
   * Get a specific state value (checks both base state and computed)
   * Supports nested keys like "user.name"
   * When accessed in Svelte components, automatically tracked by Svelte's reactivity
   */
  getValue(key: string): any {
    const keys = key.split('.');

    // For simple keys, check computed first, then state
    if (keys.length === 1) {
      if (key in this.computedValues) {
        return this.computedValues[key];
      }
      return this.state[key];
    }

    // For nested keys, traverse the object
    const rootKey = keys[0];
    let current: any = rootKey in this.computedValues
      ? this.computedValues[rootKey]
      : this.state[rootKey];

    for (let i = 1; i < keys.length; i++) {
      if (current && typeof current === 'object' && keys[i] in current) {
        current = current[keys[i]];
      } else {
        return undefined;
      }
    }

    return current;
  }

  /**
   * Reset state (clears all state)
   */
  reset() {
    this.state = {};
    console.log(`📊 [TemplateState] Reset (cleared all state)`);
  }

  /**
   * Check if a state key exists
   */
  has(key: string): boolean {
    return this.getValue(key) !== undefined;
  }

  /**
   * Get the version number - use this to track any state changes
   * When accessed in $effect, will trigger when any state property changes
   */
  getVersion(): number {
    return this.version;
  }

  /**
   * Initialize computed properties from template definition
   * Stores the definitions and computes initial values
   */
  initializeComputed(computedDefs: Record<string, string>) {
    console.log(`🧮 [TemplateState] Initializing computed properties:`, Object.keys(computedDefs));

    // Clear existing computed definitions
    this.computedDefs.clear();

    // Parse each computed property
    for (const [key, expressionString] of Object.entries(computedDefs)) {
      // Extract JEXL expression from {{...}} syntax
      const expression = extractExpression(expressionString) || expressionString;

      // Extract dependencies from the expression
      const dependencies = this.extractDependencies(expression);

      this.computedDefs.set(key, {
        expression,
        dependencies: Array.from(dependencies)
      });

      console.log(`  💡 ${key}: depends on [${Array.from(dependencies).join(', ')}]`);
    }

    // Compute initial values
    this.recomputeAll();
  }

  /**
   * Recompute all computed properties
   * Called after state changes
   */
  private recomputeAll() {
    // Skip recomputation if we're still initializing state
    if (this.isInitializing) {
      return;
    }

    for (const [key, def] of this.computedDefs.entries()) {
      try {
        const value = evaluateExpression(def.expression, this.state);
        this.computedValues[key] = value;
        console.log(`  🔄 Computed ${key} =`, value);
      } catch (error) {
        console.error(`❌ [TemplateState] Failed to compute ${key}:`, error);
        this.computedValues[key] = undefined;
      }
    }
  }

  /**
   * Extract state property dependencies from a JEXL expression
   * Returns a set of property names that the expression depends on
   */
  private extractDependencies(expression: string): Set<string> {
    const dependencies = new Set<string>();

    // Match JavaScript identifiers (variable names)
    // This regex finds potential state property references
    const identifierRegex = /\b([a-zA-Z_$][a-zA-Z0-9_$]*)\b/g;
    let match;

    while ((match = identifierRegex.exec(expression)) !== null) {
      const identifier = match[1];

      // Skip JavaScript keywords and built-in functions
      const jsKeywords = new Set([
        'true', 'false', 'null', 'undefined', 'if', 'else', 'return',
        'function', 'const', 'let', 'var', 'this', 'new', 'typeof',
        'Number', 'String', 'Boolean', 'Array', 'Object', 'Math',
        'parseInt', 'parseFloat', 'isNaN'
      ]);

      if (!jsKeywords.has(identifier)) {
        dependencies.add(identifier);
      }
    }

    return dependencies;
  }

  /**
   * Observe content and user_content Yjs documents for changes
   * Updates the merged state when either changes
   */
  observeYjsDocuments(contentMap?: Y.Map<any>, userContentMap?: Y.Map<any>) {
    // Clean up previous observers
    this.stopObserving();

    this.contentMap = contentMap || null;
    this.userContentMap = userContentMap || null;

    // Helper to merge all data sources
    const mergeAllSources = () => {
      const contentData = this.contentMap ? this.yMapToObject(this.contentMap) : {};
      const userContentData = this.userContentMap ? this.yMapToObject(this.userContentMap) : {};

      // Merge all sources into state
      const allData = {
        ...contentData,
        ...userContentData
      };

      // Update state properties
      for (const [key, value] of Object.entries(allData)) {
        this.state[key] = value;
      }

      // Increment version to trigger reactivity
      this.version++;

      // Recompute computed properties after state merge
      if (this.computedDefs.size > 0) {
        this.recomputeAll();
      }

      console.log(`📊 [TemplateState] Merged state updated from Yjs docs:`, {
        contentData,
        userContentData,
        mergedState: this.state,
        stateKeys: Object.keys(this.state)
      });

      // WARN if state is empty - might indicate data loss
      if (Object.keys(this.state).length === 0) {
        console.warn(`⚠️ [TemplateState] State is empty after merging! This might indicate:
  1. Resource was created before state persistence was implemented
  2. State was not saved to backend properly
  3. Template was not imported correctly

  To fix: Re-import the template or check backend data.`);
      }
    };

    // Observe content map changes
    if (this.contentMap) {
      const contentObserver = () => {
        console.log(`📦 [TemplateState] content_doc changed`);
        mergeAllSources();
      };
      this.contentMap.observe(contentObserver);
      this.observeHandlers.push(() => this.contentMap?.unobserve(contentObserver));
    }

    // Observe user_content map changes
    if (this.userContentMap) {
      const userContentObserver = () => {
        console.log(`👤 [TemplateState] user_content_doc changed`);
        mergeAllSources();
      };
      this.userContentMap.observe(userContentObserver);
      this.observeHandlers.push(() => this.userContentMap?.unobserve(userContentObserver));
    }

    // Initial merge
    mergeAllSources();
  }

  /**
   * Stop observing Yjs documents
   */
  stopObserving() {
    for (const unobserve of this.observeHandlers) {
      unobserve();
    }
    this.observeHandlers = [];
    this.contentMap = null;
    this.userContentMap = null;
  }

  /**
   * Convert Y.Map to plain object recursively
   */
  private yMapToObject(yMap: Y.Map<any>): Record<string, any> {
    const obj: Record<string, any> = {};
    yMap.forEach((value, key) => {
      obj[key] = value;
    });
    return obj;
  }
}

// Export singleton instance
export const templateState = new TemplateStateManager();

// Export convenience functions
export const setState = (key: string, value: any) => templateState.set(key, value);
export const updateState = (updates: Record<string, any>) => templateState.update(updates);
export const getState = () => templateState.get();
export const getStateValue = (key: string) => templateState.getValue(key);
export const resetState = () => templateState.reset();
export const initializeState = (stateDefinition?: Record<string, any>) => templateState.initialize(stateDefinition);
export const initializeComputed = (computedDefs: Record<string, string>) => templateState.initializeComputed(computedDefs);
export const observeYjsDocuments = (contentMap?: Y.Map<any>, userContentMap?: Y.Map<any>) => templateState.observeYjsDocuments(contentMap, userContentMap);
export const stopObservingYjs = () => templateState.stopObserving();
