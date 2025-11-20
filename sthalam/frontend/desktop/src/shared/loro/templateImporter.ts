/**
 * Template Importer - Simplified
 *
 * Imports HUML templates into Loro documents
 * NO tree conversions - just store HUML source and extract state
 */

import { parseHUML } from '../../lib/services/humlParser';
import { loroCoordinator } from './loroCoordinator';
import { LoroList } from 'loro-crdt';

export class TemplateImporter {
  /**
   * Import HUML template from string
   *
   * 1. Parse HUML to validate
   * 2. Store raw HUML in templateDoc
   * 3. Extract and store initial state in contentDoc
   */
  async importFromHUML(humlString: string): Promise<void> {
    console.log('📥 [TemplateImporter] Starting import...');

    // 1. Parse HUML to validate and extract structure
    const template = parseHUML(humlString);
    console.log('✅ Parsed template:', template.name);

    // 2. Get Loro maps
    const templateMap = loroCoordinator.getTemplateMap();
    const contentMap = loroCoordinator.getContentMap();

    // 3. Clear existing data
    this.clearDocuments();

    // 4. Store raw HUML source in templateDoc
    templateMap.set('huml_source', humlString);
    templateMap.set('name', template.name || 'Untitled');
    templateMap.set('version', template.version || 'v1.0.0');

    // 5. Initialize state in Loro documents from HUML template.documents section
    // Each HUML section maps to a specific Loro document

    // publisherState → content_doc (stateMap)
    if (template.documents?.publisherState) {
      const stateMap = loroCoordinator.getStateMap();
      console.log('📝 [TemplateImporter] publisherState:', template.documents.publisherState);
      this.initializeState(template.documents.publisherState, stateMap);
    }

    // contentDoc → content_doc (contentMap)
    if (template.documents?.contentDoc) {
      console.log('📝 [TemplateImporter] contentDoc:', template.documents.contentDoc);
      this.initializeState(template.documents.contentDoc, contentMap);
    }

    // collaborativeState → collaborative_doc
    if (template.documents?.collaborativeState) {
      const collaborativeMap = loroCoordinator.getCollaborativeMap();
      console.log('📝 [TemplateImporter] collaborativeState:', template.documents.collaborativeState);
      this.initializeState(template.documents.collaborativeState, collaborativeMap);
    }

    // submissionsDoc → submissions_doc
    if (template.documents?.submissionsDoc) {
      console.log('📝 [TemplateImporter] submissionsDoc:', template.documents.submissionsDoc);
      // Initialize the submissions structure directly as a list
      loroCoordinator.getDocuments().submissionsDoc.getList('submissions');
      console.log('✅ [TemplateImporter] Initialized submissions_doc with List structure');
    }

    // 6. Commit changes
    loroCoordinator.getDocuments().templateDoc.commit();
    loroCoordinator.getDocuments().contentDoc.commit();
    loroCoordinator.getDocuments().collaborativeDoc.commit();
    loroCoordinator.getDocuments().submissionsDoc.commit();

    console.log('✅ [TemplateImporter] Import complete!');
  }

  /**
   * Initialize state from template documents section
   */
  private initializeState(appState: any, contentMap: any): void {
    // Recursively initialize state from schema
    for (const [key, value] of Object.entries(appState)) {
      console.log(`🔍 [TemplateImporter] Processing field: ${key}, type: ${Array.isArray(value) ? 'Array' : typeof value}, value:`, value);

      if (this.isStateDefinition(value)) {
        // It's a state definition with type and initial value
        const def = value as any;
        if (def.initial !== undefined) {
          contentMap.set(key, def.initial);
          console.log(`✅ [TemplateImporter] Set ${key} = ${def.initial}`);
        }
      } else if (Array.isArray(value)) {
        // It's an array schema (defined with - ::)
        // Initialize as empty array
        contentMap.set(key, []);
        console.log(`✅ [TemplateImporter] Set ${key} = [] (array schema)`);
      } else if (typeof value === 'object' && value !== null) {
        // It's a nested object, recurse
        // For now, just flatten it
        // TODO: Handle nested state properly
        console.log(`⚠️ [TemplateImporter] Recursing into ${key}`);
        this.initializeState(value, contentMap);
      }
    }
  }

  /**
   * Check if value is a state definition (has type/initial)
   */
  private isStateDefinition(value: any): boolean {
    return (
      typeof value === 'object' &&
      value !== null &&
      !Array.isArray(value) &&
      ('type' in value || 'initial' in value)
    );
  }

  /**
   * Clear all documents
   */
  private clearDocuments(): void {
    console.log('🧹 Clearing documents...');

    const docs = loroCoordinator.getDocuments();

    // Clear template
    const templateMap = docs.templateDoc.getMap('template');
    templateMap.clear();

    // Clear content
    const contentMap = docs.contentDoc.getMap('content');
    contentMap.clear();

    // Note: Keep userContent, collaborative, submissions intact
  }
}

// Singleton instance
export const templateImporter = new TemplateImporter();
