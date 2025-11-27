/**
 * Document Router - Maps Permit document names to Loro documents
 *
 * Provides a dynamic routing layer between template field definitions
 * and loroCoordinator's fixed document structure.
 *
 * This allows templates to specify which Permit document each field belongs to,
 * while loroCoordinator maintains its fixed set of documents.
 */

import type { LoroMap, LoroList } from 'loro-crdt';
import type { LoroCoordinator } from './loroCoordinator';

/**
 * Maps Permit document names (from permissions.ts) to loroCoordinator getter methods
 * Note: submissions_doc returns LoroList, others return LoroMap
 */
const DOCUMENT_MAP: Record<string, (coordinator: LoroCoordinator) => LoroMap | LoroList> = {
  'template_doc': (c) => c.getTemplateMap(),
  'content_doc': (c) => c.getContentMap(),
  'user_content_doc': (c) => c.getUserContentMap(),
  'collaborative_doc': (c) => c.getCollaborativeMap(),
  'submissions_doc': (c) => c.getSubmissions(),  // Returns LoroList
  'ui_state_doc': (c) => c.getUIStateMap(),
};

/**
 * Get the Loro map or list for a given Permit document name
 * @param docName Permit document name (e.g., "content_doc", "collaborative_doc")
 * @param coordinator LoroCoordinator instance
 * @returns LoroMap or LoroList for the specified document
 * @throws Error if document name is not recognized
 */
export function getMapForDocument(docName: string, coordinator: LoroCoordinator): LoroMap | LoroList {
  const getter = DOCUMENT_MAP[docName];
  if (!getter) {
    throw new Error(`Unknown document: ${docName}. Valid documents: ${Object.keys(DOCUMENT_MAP).join(', ')}`);
  }
  return getter(coordinator);
}

/**
 * Get the Permit document name for a given field from the template
 * @param fieldName Field name (e.g., "posts", "newPostTitle")
 * @param templateDefinition Parsed template definition
 * @returns Permit document name or 'content_doc' as default
 */
export function getDocumentNameForField(fieldName: string, templateDefinition: any): string {
  const fieldDef = templateDefinition?.documents?.[fieldName];
  return fieldDef?.document || 'content_doc'; // Default to content_doc
}

/**
 * Get the Loro map or list for a given field based on template metadata
 * @param fieldName Field name
 * @param templateDefinition Parsed template definition
 * @param coordinator LoroCoordinator instance
 * @returns LoroMap or LoroList for the field's document
 */
export function getMapForField(
  fieldName: string,
  templateDefinition: any,
  coordinator: LoroCoordinator
): LoroMap | LoroList {
  const docName = getDocumentNameForField(fieldName, templateDefinition);
  return getMapForDocument(docName, coordinator);
}

/**
 * List all supported Permit document names
 */
export function getSupportedDocuments(): string[] {
  return Object.keys(DOCUMENT_MAP);
}
