/**
 * Document Router - Maps UCAN document names to Loro documents
 *
 * Provides a dynamic routing layer between template field definitions
 * and loroCoordinator's fixed document structure.
 *
 * This allows templates to specify which UCAN document each field belongs to,
 * while loroCoordinator maintains its fixed set of documents.
 */

import type { LoroMap } from 'loro-crdt';
import type { LoroCoordinator } from './loroCoordinator';

/**
 * Maps UCAN document names (from permissions.ts) to loroCoordinator getter methods
 */
const DOCUMENT_MAP: Record<string, (coordinator: LoroCoordinator) => LoroMap> = {
  'template_doc': (c) => c.getTemplateMap(),
  'content_doc': (c) => c.getContentMap(),
  'user_content_doc': (c) => c.getUserContentMap(),
  'collaborative_doc': (c) => c.getCollaborativeMap(),
  'submissions_doc': (c) => c.getSubmissions(),
  'ui_state_doc': (c) => c.getUIStateMap(),
};

/**
 * Get the Loro map for a given UCAN document name
 * @param docName UCAN document name (e.g., "content_doc", "collaborative_doc")
 * @param coordinator LoroCoordinator instance
 * @returns LoroMap for the specified document
 * @throws Error if document name is not recognized
 */
export function getMapForDocument(docName: string, coordinator: LoroCoordinator): LoroMap {
  const getter = DOCUMENT_MAP[docName];
  if (!getter) {
    throw new Error(`Unknown document: ${docName}. Valid documents: ${Object.keys(DOCUMENT_MAP).join(', ')}`);
  }
  return getter(coordinator);
}

/**
 * Get the UCAN document name for a given field from the template
 * @param fieldName Field name (e.g., "posts", "newPostTitle")
 * @param templateDefinition Parsed template definition
 * @returns UCAN document name or 'content_doc' as default
 */
export function getDocumentNameForField(fieldName: string, templateDefinition: any): string {
  const fieldDef = templateDefinition?.documents?.[fieldName];
  return fieldDef?.document || 'content_doc'; // Default to content_doc
}

/**
 * Get the Loro map for a given field based on template metadata
 * @param fieldName Field name
 * @param templateDefinition Parsed template definition
 * @param coordinator LoroCoordinator instance
 * @returns LoroMap for the field's document
 */
export function getMapForField(
  fieldName: string,
  templateDefinition: any,
  coordinator: LoroCoordinator
): LoroMap {
  const docName = getDocumentNameForField(fieldName, templateDefinition);
  return getMapForDocument(docName, coordinator);
}

/**
 * List all supported UCAN document names
 */
export function getSupportedDocuments(): string[] {
  return Object.keys(DOCUMENT_MAP);
}
