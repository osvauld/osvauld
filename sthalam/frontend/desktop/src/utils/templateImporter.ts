import { parse } from '@huml-lang/huml';
import type { YjsDocuments } from '../lib/yjsManager';

/**
 * HUML Template Schema for Sthalam
 *
 * Templates define complete websites/apps in a declarative format
 * Users can paste HUML and auto-generate the entire application
 */

export interface TemplateBlock {
  id?: string; // Auto-generated if not provided
  type: string; // Block type (heading, text, thread, etc.)
  content?: string; // Text content
  css?: string; // Custom CSS
  name?: string; // Block name/label
  parentId?: string; // Parent block ID (auto-set for children)

  // Positioning (auto-calculated if not provided)
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  zIndex?: number;

  // Container-specific
  isEntryPoint?: boolean; // For screen-container

  // Navigation-specific
  action?: 'navigate' | 'show' | 'hide' | 'toggle';
  targetScreen?: string;
  targetContainer?: string;
  targetContainerId?: string; // Direct target container ID
  value?: any; // Field value for branching (MODE 3)

  // Form-specific
  formId?: string;
  fieldName?: string;
  submit?: boolean; // Mark button as form submit button
  required?: boolean;
  eventName?: string;
  label?: string; // Form field label
  placeholder?: string; // Form field placeholder

  // Thread-specific
  mode?: 'markdown' | 'html';
  description?: string;

  // Branching-specific
  questionId?: string;
  yesTargetId?: string;
  noTargetId?: string;

  // Children blocks (nested hierarchy)
  children?: TemplateBlock[];
}

export interface TemplateScreen {
  id: string;
  name?: string;
  isEntryPoint?: boolean;
  css?: string; // Custom CSS for the screen
  children?: TemplateBlock[];
}

export interface SthalaTemplate {
  name: string;
  resourceType?: 'website' | 'noticeboard' | 'form';
  screens?: TemplateScreen[];
  blocks?: TemplateBlock[]; // Flat list alternative to screens
}

/**
 * Import HUML template and convert to Yjs blocks
 */
export class TemplateImporter {
  private blockCounter = 0;
  private screenCounter = 0;
  private currentX = 100;
  private currentY = 100;
  private screenSpacing = 1500; // Horizontal spacing between screens

  /**
   * Parse HUML string and import into Yjs documents
   */
  async importFromHUML(humlString: string, yjsDocuments: YjsDocuments): Promise<void> {
    try {
      // Parse HUML
      const parsed = parse(humlString);
      const template = parsed as SthalaTemplate;

      console.log('📥 Importing template:', template.name);
      console.log('🔍 Template structure:', template);

      // Clear existing blocks
      yjsDocuments.blocks.clear();

      // Reset counters
      this.blockCounter = 0;
      this.screenCounter = 0;
      this.currentX = 100;
      this.currentY = 100;

      // Import screens if present
      if (template.screens && template.screens.length > 0) {
        this.importScreens(template.screens, yjsDocuments);
      }
      // Otherwise import flat blocks
      else if (template.blocks && template.blocks.length > 0) {
        this.importBlocks(template.blocks, yjsDocuments, null);
      }

      console.log('✅ Template import complete!');

    } catch (error) {
      console.error('❌ Template import failed:', error);
      throw new Error(`Failed to import template: ${error}`);
    }
  }

  /**
   * Import screens (main containers with children)
   */
  private importScreens(screens: TemplateScreen[], yjsDocuments: YjsDocuments) {
    for (const screen of screens) {
      const screenId = screen.id || this.generateId('screen');

      // Parse CSS to styles
      const parsedStyles = this.parseCSSToStyles(screen.css);

      // Create screen container
      const screenBlock = {
        id: screenId,
        type: 'screen-container',
        name: screen.name || `Screen ${this.screenCounter + 1}`,
        isEntryPoint: screen.isEntryPoint !== undefined ? screen.isEntryPoint : (this.screenCounter === 0),
        x: this.currentX,
        y: this.currentY,
        width: 1200,
        // Don't set height - let it grow with content
        zIndex: 0,
        content: '',
        styles: parsedStyles,
        css: screen.css || '', // Keep original CSS string for editing
        children: [] as string[],
      };

      // Log CSS parsing if CSS was provided
      if (screen.css) {
        console.log(`    🎨 CSS for screen "${screenBlock.name}":`, screen.css);
        console.log(`    📦 Parsed styles:`, parsedStyles);
      }

      yjsDocuments.blocks.set(screenId, screenBlock);
      console.log(`  📄 Created screen: ${screenBlock.name} at (${this.currentX}, ${this.currentY})`);

      // Import children blocks within this screen
      if (screen.children && screen.children.length > 0) {
        const childIds = this.importBlocks(
          screen.children,
          yjsDocuments,
          screenId,
          this.currentX + 50,
          this.currentY + 80
        );

        // Update screen's children list
        screenBlock.children = childIds;
        yjsDocuments.blocks.set(screenId, screenBlock);
      }

      // Move to next screen position
      this.currentX += this.screenSpacing;
      this.screenCounter++;
    }
  }

  /**
   * Import blocks recursively (handles children)
   */
  private importBlocks(
    blocks: TemplateBlock[],
    yjsDocuments: YjsDocuments,
    parentId: string | null,
    startX = 100,
    startY = 100
  ): string[] {
    const createdIds: string[] = [];
    let currentY = startY;

    for (const block of blocks) {
      const blockId = block.id || this.generateId(block.type);

      // Calculate position
      const x = block.x ?? startX;
      const y = block.y ?? currentY;
      const width = block.width ?? this.getDefaultWidth(block.type);

      // Don't set height for containers - let them grow with content
      const isContainer = block.type === 'screen-container' || block.type === 'section-container';
      const height = isContainer ? undefined : (block.height ?? this.getDefaultHeight(block.type));

      const zIndex = block.zIndex ?? 1;

      // Parse CSS to styles
      const parsedStyles = this.parseCSSToStyles(block.css);

      // Create block data
      const blockData: any = {
        id: blockId,
        type: block.type,
        x,
        y,
        width,
        zIndex,
        content: block.content || '',
        styles: parsedStyles,
        css: block.css || '', // Keep original CSS string for editing
        parentId: parentId,
      };

      // Only set height for non-container blocks
      if (height !== undefined) {
        blockData.height = height;
      }

      // Log CSS parsing if CSS was provided
      if (block.css) {
        console.log(`    🎨 CSS for ${block.type}:`, block.css);
        console.log(`    📦 Parsed styles:`, parsedStyles);
      }

      // Add type-specific fields
      if (block.name) blockData.name = block.name;
      if (block.isEntryPoint !== undefined) blockData.isEntryPoint = block.isEntryPoint;
      if (block.action) blockData.action = block.action;

      // Navigation target - NavButton expects targetContainerId
      if (block.targetContainerId) {
        blockData.targetContainerId = block.targetContainerId;
      } else if (block.targetScreen) {
        blockData.targetContainerId = block.targetScreen;
      } else if (block.targetContainer) {
        blockData.targetContainerId = block.targetContainer;
      }

      if (block.formId) blockData.formId = block.formId;
      if (block.fieldName) blockData.fieldName = block.fieldName;
      if (block.value !== undefined) blockData.value = block.value; // For nav-button field values
      if (block.submit !== undefined) blockData.submit = block.submit; // For nav-button submit
      if (block.required !== undefined) blockData.required = block.required;
      if (block.eventName) blockData.eventName = block.eventName;
      if (block.mode) blockData.mode = block.mode;
      if (block.description) blockData.description = block.description;
      if (block.questionId) blockData.questionId = block.questionId;
      if (block.yesTargetId) blockData.yesTargetId = block.yesTargetId;
      if (block.noTargetId) blockData.noTargetId = block.noTargetId;

      // Form field properties
      if (block.label) blockData.label = block.label;
      if (block.placeholder) blockData.placeholder = block.placeholder;

      // Handle children
      if (block.children && block.children.length > 0) {
        const childIds = this.importBlocks(
          block.children,
          yjsDocuments,
          blockId,
          x + 20,
          y + 60
        );
        blockData.children = childIds;
      } else {
        blockData.children = [];
      }

      // Save to Yjs
      yjsDocuments.blocks.set(blockId, blockData);
      createdIds.push(blockId);

      console.log(`    ✓ Created ${block.type}: ${block.name || blockId}`);

      // Move down for next block
      currentY += height || 0 + 20;
      this.blockCounter++;
    }

    return createdIds;
  }

  /**
   * Parse CSS string to styles object
   */
  private parseCSSToStyles(css?: string): Record<string, any> {
    if (!css) return {};

    const styles: Record<string, any> = {};

    // Simple CSS parser (for inline styles)
    const declarations = css.split(';').filter(d => d.trim());

    for (const declaration of declarations) {
      const [property, value] = declaration.split(':').map(s => s.trim());
      if (property && value) {
        // Convert kebab-case to camelCase
        const camelProperty = property.replace(/-([a-z])/g, (g) => g[1].toUpperCase());
        styles[camelProperty] = value;
      }
    }

    return styles;
  }

  /**
   * Generate unique block ID
   */
  private generateId(type: string): string {
    return `${type}-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
  }

  /**
   * Get default width for block type
   */
  private getDefaultWidth(type: string): number {
    const widths: Record<string, number> = {
      'screen-container': 1200,
      'section-container': 800,
      'heading': 600,
      'text': 600,
      'markdown-text': 600,
      'image': 400,
      'thread': 800,
      'form': 600,
      'form-field-text': 300,
      'form-field-textarea': 400,
      'form-field-checkbox': 200,
      'form-submit-button': 150,
      'nav-button': 150,
      'branching-question': 500,
    };

    return widths[type] || 400;
  }

  /**
   * Get default height for block type
   */
  private getDefaultHeight(type: string): number {
    const heights: Record<string, number> = {
      'screen-container': 800,
      'section-container': 400,
      'heading': 60,
      'text': 100,
      'markdown-text': 200,
      'image': 300,
      'thread': 600,
      'form': 50,
      'form-field-text': 60,
      'form-field-textarea': 120,
      'form-field-checkbox': 40,
      'form-submit-button': 50,
      'nav-button': 50,
      'branching-question': 200,
    };

    return heights[type] || 100;
  }
}
