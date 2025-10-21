# Sthalam Template System

## Overview

The template system allows users to create entire websites by pasting HUML (Human-oriented Markup Language) templates. This makes it easy to:
- Share website designs
- Quick-start projects
- Learn by example
- Build complex layouts without manual block placement

## Architecture

### Components

1. **Template Importer** (`src/utils/templateImporter.ts`)
   - Parses HUML using `@huml-lang/huml` package
   - Converts template structure to Yjs blocks
   - Handles parent-child relationships
   - Auto-generates positions and IDs

2. **Import Modal** (`src/components/TemplateImportModal.svelte`)
   - User interface for pasting templates
   - Example template loading
   - Error handling and validation
   - Success feedback

3. **Block Palette Integration** (`src/lib/BlockPalette.svelte`)
   - "Import Template" button in footer
   - Triggers modal on click

4. **WebsiteBuilder Integration** (`src/lib/WebsiteBuilder.svelte`)
   - Manages import modal state
   - Passes Yjs documents to importer

### File Structure

```
sthalam/
├── frontend/desktop/src/
│   ├── utils/
│   │   └── templateImporter.ts          # HUML → Yjs converter
│   ├── components/
│   │   └── TemplateImportModal.svelte   # Import UI
│   └── lib/
│       ├── BlockPalette.svelte          # Updated with import button
│       └── WebsiteBuilder.svelte        # Updated with modal
│
└── templates/
    ├── README.md                         # Documentation
    ├── dev-blog-basic.huml              # Basic blog template
    └── dev-blog-enhanced.huml           # Enhanced blog template
```

## HUML Template Schema

### Root Structure

```typescript
interface SthalaTemplate {
  name: string;                    // Template name
  resourceType?: 'website' | 'noticeboard' | 'form';
  screens?: TemplateScreen[];      // Screen-based layout
  blocks?: TemplateBlock[];        // Or flat block list
}
```

### Screen Structure

```typescript
interface TemplateScreen {
  id: string;                      // Screen ID
  name?: string;                   // Display name
  isEntryPoint?: boolean;          // Entry point flag
  children?: TemplateBlock[];      // Nested blocks
}
```

### Block Structure

```typescript
interface TemplateBlock {
  id?: string;                     // Auto-generated if omitted
  type: string;                    // Block type
  content?: string;                // Text content
  css?: string;                    // Custom CSS
  name?: string;                   // Block name

  // Positioning (auto-calculated)
  x?: number;
  y?: number;
  width?: number;
  height?: number;

  // Type-specific properties
  action?: 'navigate' | 'show' | 'hide' | 'toggle';
  targetScreen?: string;
  formId?: string;
  // ... etc

  children?: TemplateBlock[];      // Nested children
}
```

## How It Works

### 1. User Flow

```
User clicks "Import Template"
  ↓
Modal opens
  ↓
User pastes HUML template
  ↓
Clicks "Import Template" button
  ↓
HUML parsed → Yjs blocks created
  ↓
Canvas updates with new blocks
  ↓
Success! Modal closes
```

### 2. Import Process

```typescript
// 1. Parse HUML
const parsed = parse(humlString);

// 2. Clear existing blocks
yjsDocuments.blocks.clear();

// 3. Import screens/blocks recursively
importScreens(template.screens);
  → importBlocks(children, parentId)
    → Creates Yjs block with:
      - Auto-generated ID
      - Calculated position
      - Parsed CSS → styles object
      - Parent-child relationships

// 4. Yjs automatically syncs changes
```

### 3. Position Calculation

- **Screens**: Placed horizontally with 1500px spacing
  - Screen 1: x=100
  - Screen 2: x=1600
  - Screen 3: x=3100

- **Children**: Nested inside parent with offsets
  - Child x = parent.x + 20
  - Child y = parent.y + 60

- **Sequential blocks**: Stacked vertically
  - Next y = previous.y + previous.height + 20

## Example Template

```huml
name: "Simple Blog"
resourceType: "website"

screens::
  - id: "home"
    name: "Home"
    isEntryPoint: true
    children::
      - type: "heading"
        content: "My Blog"
        css: "font-size: 48px; color: #89b4fa;"

      - type: "section-container"
        css: "display: flex; gap: 20px;"
        children::
          - type: "text"
            content: "Welcome to my blog!"

          - type: "nav-button"
            content: "Read More"
            action: "navigate"
            targetScreen: "post-1"

  - id: "post-1"
    name: "First Post"
    children::
      - type: "thread"
        content: "# My First Post\n\nHello world!"

      - type: "nav-button"
        content: "← Back"
        action: "navigate"
        targetScreen: "home"
```

## Supported Block Types

All standard Sthalam blocks are supported:

- `screen-container` - Main page container
- `section-container` - Layout section
- `heading` - Title text
- `text` - Paragraph
- `markdown-text` - Rich text
- `image` - Image/GIF
- `thread` - Blog post with comments
- `form` - Form metadata
- `form-field-text` - Text input
- `form-field-textarea` - Text area
- `form-field-checkbox` - Checkbox
- `form-submit-button` - Submit button
- `nav-button` - Navigation button
- `branching-question` - Yes/No question

## CSS Handling

CSS is parsed from string to styles object:

```huml
css: "font-size: 48px; color: #89b4fa;"

# Becomes:
{
  fontSize: "48px",
  color: "#89b4fa"
}
```

Multi-line CSS uses triple quotes:

```huml
css: """
  display: flex;
  flex-direction: column;
  gap: 20px;
"""
```

## Future Enhancements

### Export Functionality
- Add "Export Template" button
- Convert current Yjs blocks → HUML
- Copy to clipboard or download file

### Template Library
- Built-in template browser
- Categories (blog, landing page, form, etc.)
- Preview thumbnails
- One-click import

### Template Variables
```huml
variables::
  primaryColor: "#89b4fa"
  siteName: "My Website"

# Then use:
content: "{{siteName}}"
css: "color: {{primaryColor}};"
```

### Conditional Blocks
```huml
- type: "section"
  if: "showBanner"
  children:: ...
```

## Debugging

### Enable Console Logging

The importer logs progress:
```
📥 Importing template: Dev Confession Blog
  📄 Created screen: Home at (100, 100)
    ✓ Created heading: DEV CONFESSIONS
    ✓ Created section-container: blog-cards
✅ Template import complete!
```

### Common Issues

**Parse Error:**
- Check HUML syntax (2-space indentation)
- Ensure colons are correct (`:` vs `::`)
- Validate triple quotes for multi-line

**Blocks Not Visible:**
- Must have `isEntryPoint: true` on at least one screen
- Check block positioning (x, y)
- Verify parent-child relationships

**Navigation Fails:**
- Ensure `targetScreen` matches a screen `id`
- Check screen IDs are unique

## Testing

To test the system:

1. Start Sthalam: `cd sthalam/src-tauri && cargo tauri dev`
2. Create or select a resource
3. Click "📥 Import Template" in Block Palette
4. Load example or paste a template
5. Click "Import Template"
6. Verify blocks appear on canvas
7. Test navigation between screens
8. Check thread blocks and comments

## Performance

- Templates with 50+ blocks import instantly
- Position calculation is O(n) where n = number of blocks
- Yjs efficiently handles batch updates
- No noticeable lag on import

## Security

- HUML parsing is safe (no code execution)
- CSS is applied as inline styles (no `<script>` injection)
- Thread content uses DOMPurify for sanitization

---

## Quick Start for Users

1. **Get a template** from `sthalam/templates/`
2. **Copy the entire file content**
3. **In Sthalam**, click "📥 Import Template"
4. **Paste** and click "Import Template"
5. **Done!** Your website is ready

Try `dev-blog-basic.huml` first to see the system in action!
