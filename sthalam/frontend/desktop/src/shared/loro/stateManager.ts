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

/**
 * Initialize state from template documents
 * Loads initial values and persisted values from Loro documents
 *
 * @param template Parsed HUML template
 * @param loroCoordinator LoroCoordinator instance
 * @returns Initialized state object
 */
export function initializeStateFromTemplate(
  template: any,
  loroCoordinator: LoroCoordinator
): Record<string, any> {
  const documentsDefinition = template.documents || {};

  // Build initial state and load persisted values from Loro documents
  const initialState: Record<string, any> = {};
  const persistedState: Record<string, any> = {};

  for (const [fieldName, fieldDef] of Object.entries(documentsDefinition)) {
    if (typeof fieldDef !== 'object' || fieldDef === null) continue;

    // Get initial value from field definition
    if ('initial' in fieldDef) {
      initialState[fieldName] = (fieldDef as any).initial;
    }

    // Load persisted value from the field's Loro document
    try {
      const loroMap = getMapForField(fieldName, template, loroCoordinator);
      const value = loroMap.get(fieldName);
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
  const loroMap = getMapForDocument(documentName, loroCoordinator);
  const fields = getFieldsForDocument(template, documentName);

  for (const fieldName of fields) {
    const value = loroMap.get(fieldName);
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
