/**
 * Template Importer - Parse HUML and create Loro CRDT structure
 *
 * Converts HUML templates into Loro documents with:
 * - Tree structure for blocks
 * - LoroText for all text content (efficient sync)
 * - Dynamic state mapping
 * - Computed expressions
 */

import { parse } from '@huml-lang/huml';
import type { LoroTree, LoroMap, LoroText, TreeID } from 'loro-crdt';
import { loroCoordinator } from './loroCoordinator';
import type { Block } from '../types/block.types';

/**
 * HUML Template Structure V3
 */
export interface HUMLTemplate {
  name: string;
  resourceType?: 'website' | 'form' | 'dashboard';

  // Shared persistent content (both modes can read)
  content?: Record<string, any>;

  // Publisher mode template
  publisher?: {
    state?: Record<string, any>;        // Publisher UI state
    computed?: Record<string, string>;  // Publisher computed values
    screens?: Array<{
      id: string;
      name?: string;
      isEntryPoint?: boolean;
      css?: string;
      uiState?: string;
      blocks?: any[];
    }>;
  };

  // Viewer mode template
  viewer?: {
    state?: Record<string, any>;        // Viewer UI state
    computed?: Record<string, string>;  // Viewer computed values
    screens?: Array<{
      id: string;
      name?: string;
      isEntryPoint?: boolean;
      css?: string;
      uiState?: string;
      blocks?: any[];
    }>;
  };

  // Legacy support (for old templates)
  state?: Record<string, any>;
  user_content?: Record<string, any>;
  computed?: Record<string, string>;
  uiStateTree?: Record<string, any>;
  screens?: Array<{
    id: string;
    name?: string;
    isEntryPoint?: boolean;
    css?: string;
    uiState?: string;
    blocks?: any[];
  }>;
  blocks?: any[];
}

/**
 * Template Importer Class
 */
export class TemplateImporter {
  private blockIdCounter = 0;
  private screenIdMap = new Map<string, TreeID>(); // Human ID → TreeID mapping
  private blockIdMap = new Map<string, TreeID>();  // For targetContainerId references

  /**
   * Main import function - parse HUML and populate Loro documents
   */
  async importFromHUML(humlString: string): Promise<void> {
    try {
      console.log('📥 [TemplateImporter] Starting import...');

      // 1. Parse HUML string
      const parsed = parse(humlString);
      const template = parsed as HUMLTemplate;

      console.log('✅ Parsed template:', template.name);
      console.log('📊 Template structure:', {
        screens: template.screens?.length || 0,
        computedKeys: Object.keys(template.computed || {}),
        stateKeys: Object.keys(template.state || {}),
        contentKeys: Object.keys(template.content || {})
      });

      // 2. Get Loro documents
      const documents = loroCoordinator.getDocuments();

      // 3. Clear and setup documents
      await this.clearDocuments();

      // 4. Import in correct order
      await this.importMetadata(template);
      await this.importContent(template);

      // Check if new structure (publisher/viewer) or legacy
      if (template.publisher || template.viewer) {
        // New structure - import publisher and viewer separately
        await this.importPublisherTemplate(template);
        await this.importViewerTemplate(template);
      } else {
        // Legacy structure - import old way
        await this.importUserState(template);
        await this.importComputed(template);
        await this.importUIStateTree(template);
        await this.importScreensAndBlocks(template);
      }

      console.log('✅ [TemplateImporter] Import complete!');

    } catch (error) {
      console.error('❌ [TemplateImporter] Import failed:', error);
      throw error;
    }
  }

  /**
   * Clear all documents (start fresh)
   */
  private async clearDocuments() {
    console.log('🧹 Clearing documents...');

    // Get fresh documents and clear them
    const templateTree = loroCoordinator.getTemplateTree();
    const contentMap = loroCoordinator.getContentMap();
    const userContentMap = loroCoordinator.getUserContentMap();
    const uiStateTree = loroCoordinator.getUIStateTree();

    // Clear maps
    contentMap.clear();
    userContentMap.clear();

    // Clear ID mappings
    this.screenIdMap.clear();
    this.blockIdMap.clear();
    this.blockIdCounter = 0;
  }

  /**
   * Import template metadata
   */
  private async importMetadata(template: HUMLTemplate) {
    console.log('📦 Importing metadata...');

    const metadata = loroCoordinator.getDocuments().templateDoc.getMap('metadata');
    metadata.set('name', template.name);

    if (template.resourceType) {
      metadata.set('resourceType', template.resourceType);
    }

    metadata.set('importedAt', Date.now());
  }

  /**
   * Import content (publisher data)
   * This is shared across all users
   */
  private async importContent(template: HUMLTemplate) {
    if (!template.content) return;

    console.log('📦 Importing content:', Object.keys(template.content));

    const contentMap = loroCoordinator.getContentMap();

    for (const [key, value] of Object.entries(template.content)) {
      contentMap.set(key, value);
    }
  }

  /**
   * Import user state (dynamic, template-defined)
   * Includes both 'state' and 'user_content' from HUML
   */
  private async importUserState(template: HUMLTemplate) {
    const userContentMap = loroCoordinator.getUserContentMap();

    // Import 'state' (session state that gets persisted)
    if (template.state) {
      console.log('📦 Importing state:', Object.keys(template.state));

      for (const [key, value] of Object.entries(template.state)) {
        userContentMap.set(key, value);
      }
    }

    // Import 'user_content' (explicit per-user state)
    if (template.user_content) {
      console.log('📦 Importing user_content:', Object.keys(template.user_content));

      for (const [key, value] of Object.entries(template.user_content)) {
        userContentMap.set(key, value);
      }
    }
  }

  /**
   * Import computed expressions
   * These are CEL expressions that derive values from state
   */
  private async importComputed(template: HUMLTemplate) {
    if (!template.computed) return;

    console.log('🧮 Importing computed expressions:', Object.keys(template.computed));

    const computed = loroCoordinator.getDocuments().templateDoc.getMap('computed');

    for (const [key, expression] of Object.entries(template.computed)) {
      computed.set(key, expression);
    }
  }

  /**
   * Import UI state tree structure (session-only)
   * This defines the shape of component-specific UI state
   */
  private async importUIStateTree(template: HUMLTemplate) {
    if (!template.uiStateTree) return;

    console.log('🌲 Importing UI state tree...');

    const uiStateTree = loroCoordinator.getUIStateTree();
    this.buildUIStateTree(uiStateTree, template.uiStateTree);
  }

  /**
   * Recursively build UI state tree
   */
  private buildUIStateTree(tree: LoroTree, schema: Record<string, any>, parentNode?: any) {
    for (const [key, value] of Object.entries(schema)) {
      if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
        // Create a tree node for nested objects
        const node = parentNode ? tree.createNode(parentNode.id) : tree.createNode();
        node.data.set('_key', key);

        // Recursively build nested structure
        this.buildUIStateTree(tree, value, node);
      } else {
        // Leaf value - set in parent's data
        if (parentNode) {
          parentNode.data.set(key, value);
        }
      }
    }
  }

  /**
   * Import Publisher template
   */
  private async importPublisherTemplate(template: HUMLTemplate) {
    if (!template.publisher) return;

    console.log('📝 [TemplateImporter] Importing publisher template...');
    console.log('📋 [TemplateImporter] Publisher screens count:', template.publisher.screens?.length);
    if (template.publisher.screens && template.publisher.screens.length > 0) {
      const firstScreen = template.publisher.screens[0];
      console.log('📦 [TemplateImporter] First screen blocks:', firstScreen.blocks?.length);
      if (firstScreen.blocks && firstScreen.blocks.length > 0) {
        const firstBlock = firstScreen.blocks[0];
        console.log('🔍 [TemplateImporter] First block:', {
          type: firstBlock.type,
          childBlocks: firstBlock.blocks?.length || 0
        });
      }
    }

    const metadata = loroCoordinator.getDocuments().templateDoc.getMap('metadata');
    metadata.set('hasPublisher', true);

    // Store publisher state definition
    if (template.publisher.state) {
      const publisherState = loroCoordinator.getDocuments().templateDoc.getMap('publisherState');
      for (const [key, value] of Object.entries(template.publisher.state)) {
        publisherState.set(key, value);
      }
    }

    // Store publisher computed expressions
    if (template.publisher.computed) {
      const publisherComputed = loroCoordinator.getDocuments().templateDoc.getMap('publisherComputed');
      for (const [key, expression] of Object.entries(template.publisher.computed)) {
        publisherComputed.set(key, expression);
      }
    }

    // Import publisher screens
    if (template.publisher.screens && template.publisher.screens.length > 0) {
      const publisherTree = loroCoordinator.getDocuments().templateDoc.getTree('publisherScreens');
      for (let i = 0; i < template.publisher.screens.length; i++) {
        const screen = template.publisher.screens[i];
        await this.importScreen(publisherTree, screen, i === 0);
      }
    }
  }

  /**
   * Import Viewer template
   */
  private async importViewerTemplate(template: HUMLTemplate) {
    if (!template.viewer) return;

    console.log('👀 [TemplateImporter] Importing viewer template...');

    const metadata = loroCoordinator.getDocuments().templateDoc.getMap('metadata');
    metadata.set('hasViewer', true);

    // Store viewer state definition
    if (template.viewer.state) {
      const viewerState = loroCoordinator.getDocuments().templateDoc.getMap('viewerState');
      for (const [key, value] of Object.entries(template.viewer.state)) {
        viewerState.set(key, value);
      }
    }

    // Store viewer computed expressions
    if (template.viewer.computed) {
      const viewerComputed = loroCoordinator.getDocuments().templateDoc.getMap('viewerComputed');
      for (const [key, expression] of Object.entries(template.viewer.computed)) {
        viewerComputed.set(key, expression);
      }
    }

    // Import viewer screens
    if (template.viewer.screens && template.viewer.screens.length > 0) {
      const viewerTree = loroCoordinator.getDocuments().templateDoc.getTree('viewerScreens');
      for (let i = 0; i < template.viewer.screens.length; i++) {
        const screen = template.viewer.screens[i];
        await this.importScreen(viewerTree, screen, i === 0);
      }
    }
  }

  /**
   * Import screens and blocks (main UI structure)
   */
  private async importScreensAndBlocks(template: HUMLTemplate) {
    const templateTree = loroCoordinator.getTemplateTree();

    if (template.screens && template.screens.length > 0) {
      console.log('📄 Importing screens:', template.screens.length);

      for (let i = 0; i < template.screens.length; i++) {
        const screen = template.screens[i];
        await this.importScreen(templateTree, screen, i === 0);
      }
    } else if (template.blocks && template.blocks.length > 0) {
      console.log('📦 Importing flat blocks (creating default screen)...');

      // Create a default screen for flat blocks
      const screenNode = templateTree.createNode();

      screenNode.data.set('id', 'main');
      screenNode.data.set('type', 'screen-container');
      screenNode.data.set('name', 'Main Screen');
      screenNode.data.set('isEntryPoint', true);
      screenNode.data.set('treeId', screenNode.id);

      this.screenIdMap.set('main', screenNode.id);
      this.blockIdMap.set('main', screenNode.id);

      // Import blocks under this screen
      await this.importBlocks(templateTree, screenNode, template.blocks);
    }
  }

  /**
   * Import a screen with its blocks
   */
  private async importScreen(tree: LoroTree, screen: any, isFirst: boolean) {
    console.log(`  📄 Importing screen: ${screen.name || screen.id}`);

    // Create screen node
    const screenNode = tree.createNode();

    // Set screen properties
    screenNode.data.set('id', screen.id);
    screenNode.data.set('type', 'screen-container');
    screenNode.data.set('name', screen.name || screen.id);
    screenNode.data.set('isEntryPoint', screen.isEntryPoint ?? isFirst);
    screenNode.data.set('treeId', screenNode.id);

    if (screen.css) {
      screenNode.data.set('css', screen.css);
    }

    if (screen.uiState) {
      screenNode.data.set('uiStateRef', screen.uiState);
    }

    // Store ID mapping for references
    this.screenIdMap.set(screen.id, screenNode.id);
    this.blockIdMap.set(screen.id, screenNode.id);

    // Import child blocks
    const blocks = screen.blocks || screen.children || [];
    if (blocks.length > 0) {
      await this.importBlocks(tree, screenNode, blocks);
    }
  }

  /**
   * Import blocks recursively
   */
  private async importBlocks(tree: LoroTree, parentNode: any, blocks: any[]) {
    for (const block of blocks) {
      await this.importBlock(tree, parentNode, block);
    }
  }

  /**
   * Import a single block with all its properties
   */
  private async importBlock(tree: LoroTree, parentNode: any, block: any) {
    const childCount = (block.blocks || block.children || []).length;
    console.log(`    ✓ Importing ${block.type}: ${block.name || '(unnamed)'} (${childCount} children)`);

    // Create block node
    const blockNode = tree.createNode(parentNode.id);

    // Generate or use existing ID
    const blockId = block.id || this.generateBlockId(block.type);

    // Set core properties
    blockNode.data.set('id', blockId);
    blockNode.data.set('type', block.type);
    blockNode.data.set('treeId', blockNode.id);

    // Store ID mapping for targetContainerId references
    if (block.id) {
      this.blockIdMap.set(block.id, blockNode.id);
    }

    // Set optional properties (all dynamic from template)
    const simpleProps = [
      'name', 'css', 'visible', 'uiState', 'uiStateRef',
      'action', 'target', 'stateKey', 'stateValue', 'stateUpdates',
      'targetContainerId', 'targetScreen', 'targetContainer',
      'formId', 'fieldName', 'submit', 'required', 'label', 'placeholder',
      'mode', 'description', 'forEach', 'forEachAs',
      'src', 'alt', 'value', 'options', 'eventName', 'submitEndpoint',
      'isEntryPoint', 'isModal', 'dataId'
    ];

    for (const prop of simpleProps) {
      if (block[prop] !== undefined) {
        blockNode.data.set(prop, block[prop]);
      }
    }

    // Handle text content
    if (block.content !== undefined) {
      blockNode.data.set('content', block.content);
    }

    // Process navigation targets
    if (!block.targetContainerId) {
      if (block.targetScreen) {
        blockNode.data.set('targetContainerId', block.targetScreen);
      } else if (block.targetContainer) {
        blockNode.data.set('targetContainerId', block.targetContainer);
      }
    }

    // Import child blocks recursively
    const children = block.blocks || block.children || [];
    if (children.length > 0) {
      await this.importBlocks(tree, blockNode, children);
    }
  }

  /**
   * Generate unique block ID
   */
  private generateBlockId(type: string): string {
    this.blockIdCounter++;
    return `${type}-${Date.now()}-${this.blockIdCounter}`;
  }

  /**
   * Get ID mappings (useful for debugging and navigation)
   */
  getIdMappings() {
    return {
      screens: this.screenIdMap,
      blocks: this.blockIdMap
    };
  }
}

// Export singleton instance
export const templateImporter = new TemplateImporter();