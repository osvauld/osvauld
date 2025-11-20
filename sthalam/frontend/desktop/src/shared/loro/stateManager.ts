/**
 * State Manager - Shared state initialization and synchronization logic
 *
 * Provides common functionality for both PublisherApp and ViewerApp to:
 * - Initialize state from template documents
 * - Load persisted values from Loro documents
 * - Subscribe to document changes
 * - Update state when documents sync
 */

import type { LoroCoordinator } from './loroCoordinator';
import { getMapForField, getMapForDocument } from './documentRouter';
import { evaluateCEL } from '../../lib/services/celEvaluator';

/**
 * Initialize state from template documents
 * Loads initial values and persisted values from Loro documents
 *
 * @param template Parsed HUML template
 * @param loroCoordinator LoroCoordinator instance
 * @param context Optional context for evaluating CEL expressions in initial values (e.g., userId, deviceId)
 * @returns Initialized state object
 */
export function initializeStateFromTemplate(
  template: any,
  loroCoordinator: LoroCoordinator,
  context?: Record<string, any>
): Record<string, any> {
  const documentsDefinition = template.documents || {};

  // Build initial state and load persisted values from Loro documents
  const initialState: Record<string, any> = {};
  const persistedState: Record<string, any> = {};

  for (const [fieldName, fieldDef] of Object.entries(documentsDefinition)) {
    if (typeof fieldDef !== 'object' || fieldDef === null) continue;

    // Get initial value from field definition
    if ('initial' in fieldDef) {
      let initialValue = (fieldDef as any).initial;

      // Evaluate CEL expressions in initial values if context is provided
      if (context && typeof initialValue === 'string' && initialValue.includes('${')) {
        try {
          const result = evaluateCEL(initialValue, context);
          if (result !== null && result !== undefined) {
            initialValue = result;
            console.log(`✅ [StateManager] Evaluated initial value for '${fieldName}':`, initialValue);
          }
        } catch (error) {
          console.warn(`⚠️ [StateManager] Failed to evaluate initial value for '${fieldName}':`, error);
          // Keep the literal string if evaluation fails
        }
      }

      initialState[fieldName] = initialValue;
    }

    // Load persisted value from the field's Loro document
    try {
      const loroContainer = getMapForField(fieldName, template, loroCoordinator);

      // Check if it's a LoroList (for submissions) or LoroMap (for other fields)
      let value;
      if (typeof (loroContainer as any).toJSON === 'function' && fieldName === 'submissions') {
        // It's a LoroList - use toJSON() to get the array
        value = (loroContainer as any).toJSON();
      } else {
        // It's a LoroMap - use get() to retrieve the value
        value = (loroContainer as any).get(fieldName);
      }

      if (value !== undefined) {
        persistedState[fieldName] = value;
      }
    } catch (error) {
      // Field might not have a document mapping or might not be persisted yet
      console.debug(`[StateManager] Could not load persisted value for '${fieldName}':`, error);
    }
  }

  // Merge: initial state < persisted state
  return { ...initialState, ...persistedState };
}

/**
 * Get fields that belong to a specific document
 *
 * @param template Parsed HUML template
 * @param documentName UCAN document name (e.g., "content_doc", "collaborative_doc")
 * @returns Array of field names that belong to this document
 */
export function getFieldsForDocument(template: any, documentName: string): string[] {
  const documentsDefinition = template.documents || {};
  const fields: string[] = [];

  for (const [fieldName, fieldDef] of Object.entries(documentsDefinition)) {
    if (typeof fieldDef === 'object' && fieldDef !== null && (fieldDef as any).document === documentName) {
      fields.push(fieldName);
    }
  }

  return fields;
}

/**
 * Reload fields from a specific Loro document
 * Used when a document changes from sync
 *
 * @param documentName UCAN document name
 * @param template Parsed HUML template
 * @param loroCoordinator LoroCoordinator instance
 * @returns Updated state object with values from the document
 */
export function reloadFieldsFromDocument(
  documentName: string,
  template: any,
  loroCoordinator: LoroCoordinator
): Record<string, any> {
  const updatedState: Record<string, any> = {};
  const loroContainer = getMapForDocument(documentName, loroCoordinator);
  const fields = getFieldsForDocument(template, documentName);

  for (const fieldName of fields) {
    let value;

    // Special handling for submissions (LoroList)
    if (fieldName === 'submissions' && documentName === 'submissions_doc') {
      value = (loroContainer as any).toJSON();
    } else {
      value = (loroContainer as any).get(fieldName);
    }

    if (value !== undefined) {
      updatedState[fieldName] = value;
    }
  }

  return updatedState;
}

/**
 * Setup subscription to a Loro document
 * Calls callback when the document changes
 *
 * @param documentName UCAN document name
 * @param loroCoordinator LoroCoordinator instance
 * @param callback Callback to invoke when document changes
 * @returns Unsubscribe function
 */
export function subscribeToDocument(
  documentName: string,
  loroCoordinator: LoroCoordinator,
  callback: () => void
): () => void {
  // Map UCAN document names to Loro document instances
  const docMap: Record<string, keyof ReturnType<typeof loroCoordinator.getDocuments>> = {
    'template_doc': 'templateDoc',
    'content_doc': 'contentDoc',
    'user_content_doc': 'userContentDoc',
    'collaborative_doc': 'collaborativeDoc',
    'submissions_doc': 'submissionsDoc',
    'ui_state_doc': 'uiStateDoc',
  };

  const loroDocName = docMap[documentName];
  if (!loroDocName) {
    console.warn(`[StateManager] Unknown document: ${documentName}`);
    return () => {};
  }

  const loroDoc = loroCoordinator.getDocuments()[loroDocName];
  return loroDoc.subscribe(callback);
}

/**
 * Get all unique document names used in the template
 *
 * @param template Parsed HUML template
 * @returns Array of unique UCAN document names
 */
export function getUniqueDocuments(template: any): string[] {
  const documentsDefinition = template.documents || {};
  const documentNames = new Set<string>();

  for (const [fieldName, fieldDef] of Object.entries(documentsDefinition)) {
    if (typeof fieldDef === 'object' && fieldDef !== null && (fieldDef as any).document) {
      documentNames.add((fieldDef as any).document);
    }
  }

  return Array.from(documentNames);
}

/**
 * Check if a document should trigger reactive updates
 *
 * @param documentName UCAN document name
 * @returns True if this document should trigger reactive signal updates
 */
export function shouldTriggerReactiveUpdate(documentName: string): boolean {
  // Only trigger reactive updates for synced documents
  // UI-only documents (ui_state_doc) don't need reactive updates
  return documentName === 'content_doc' || documentName === 'collaborative_doc';
}
