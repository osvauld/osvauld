# Viewer Mode Implementation Guide

**Date**: 2025-11-10
**Status**: ✅ Complete (Phases 1-4)

---

## Overview

This document describes the implementation of the unified navigation panel and viewer mode rendering in Sthalam.

---

## Architecture Changes

### Three Modes in Sthalam

Sthalam now has three distinct modes, all sharing a unified NavigationPanel:

```
┌─────────────────────────────────────────────────────────┐
│                  Sthalam Application                    │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌─────────────────┐  ┌─────────────────────────────┐ │
│  │ NavigationPanel │  │     Mode-Specific Content   │ │
│  │   (Unified)     │  │                             │ │
│  │                 │  │  • BuilderApp (HUML Editor) │ │
│  │ • ModeSwitcher  │  │  • PublisherApp (Create)    │ │
│  │ • Mode Actions  │  │  • ViewerApp (Consume)      │ │
│  │ • Folder List   │  │                             │ │
│  │ • Collapsible   │  │                             │ │
│  └─────────────────┘  └─────────────────────────────┘ │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

#### 1. **Builder Mode**
- **Purpose**: Edit HUML templates (source code)
- **UI**: CodeMirror editor with syntax highlighting
- **NavigationPanel**:
  - Shows "Add Sovereign Node" button
  - Uses `WebsiteFolder` component
  - Save, Sync, Publish buttons visible

#### 2. **Publisher Mode**
- **Purpose**: Create content using interactive forms
- **UI**: Renders `ui.publisher` screens from HUML
- **NavigationPanel**:
  - Shows "Add Sovereign Node" button
  - Uses `WebsiteFolder` component
  - Full editing capabilities

#### 3. **Viewer Mode** (NEW)
- **Purpose**: View published content (read-only)
- **UI**: Renders `ui.viewer` screens from HUML
- **NavigationPanel**:
  - Shows "Add Website" button (connects to published sites)
  - Uses `ViewerWebsiteFolder` component (with sync)
  - Read-only experience

---

## NavigationPanel - Mode-Aware Design

### Unified Panel for All Modes

The `NavigationPanel` component now adapts to the current mode:

**File**: `/frontend/desktop/src/components/NavigationPanel.svelte`

```typescript
const currentMode = $derived(uiState.mode); // "builder" | "publisher" | "viewer"
```

### Mode-Specific UI Elements

| Element | Builder | Publisher | Viewer |
|---------|---------|-----------|--------|
| **Add Button** | Add Sovereign Node | Add Sovereign Node | Add Website |
| **Folder Component** | WebsiteFolder | WebsiteFolder | ViewerWebsiteFolder |
| **Modal** | AddSovereignNodeModal | AddSovereignNodeModal | AddWebsiteConnectionModal |
| **Save Button** | ✓ Visible | ✓ Visible | ✓ Visible |
| **Sync Button** | ✓ Visible | ✓ Visible | ✓ Visible |
| **Publish Button** | ✓ Visible | ✓ Visible | ✓ Visible |

---

## Viewer Mode Architecture

### Template Rendering Flow

```
┌──────────────────────────────────────────────────────────┐
│  1. Load HUML Template from templateDoc                 │
└────────────────────┬─────────────────────────────────────┘
                     │
┌────────────────────▼─────────────────────────────────────┐
│  2. Parse HUML (using humlParser WASM)                   │
│     - Extract ui.viewer screens                          │
│     - Extract viewerState definitions                    │
│     - Extract viewerComputed expressions                 │
└────────────────────┬─────────────────────────────────────┘
                     │
┌────────────────────▼─────────────────────────────────────┐
│  3. Initialize Viewer State (Read-Only)                  │
│     - Load initial state from template                   │
│     - Load persisted state from contentDoc               │
│     - Evaluate computed expressions (CEL)                │
└────────────────────┬─────────────────────────────────────┘
                     │
┌────────────────────▼─────────────────────────────────────┐
│  4. Render Screens with BlockRenderer                    │
│     - ScreenRenderer receives screens + context          │
│     - BlockRenderer renders each block recursively       │
│     - Apply CSS styling from template                    │
└────────────────────┬─────────────────────────────────────┘
                     │
┌────────────────────▼─────────────────────────────────────┐
│  5. Handle User Actions (Limited)                        │
│     - navigate: Switch between screens ✓                 │
│     - setState: Blocked (read-only) ✗                    │
│     - uploads: Blocked (read-only) ✗                     │
└──────────────────────────────────────────────────────────┘
```

### Key Files

#### ViewerApp.svelte
**Path**: `/frontend/desktop/src/viewer/ViewerApp.svelte`

**Responsibilities**:
- Parse HUML and extract `ui.viewer` screens
- Initialize viewer state (read-only)
- Evaluate computed expressions
- Handle navigation actions
- Subscribe to template changes
- Render ScreenRenderer with current screen

**Pattern**: Follows same structure as `PublisherApp.svelte`

#### ScreenRenderer.svelte
**Path**: `/frontend/desktop/src/viewer/ScreenRenderer.svelte`

**Responsibilities**:
- Receive current screen from ViewerApp
- Iterate through screen blocks
- Render each block using BlockRenderer
- Block state changes (read-only)

---

## HUML Template Structure

### ui.viewer vs ui.publisher

Templates can define separate UI for publishers (creators) and viewers (consumers):

```yaml
name: "My Template"
version: "v1.0.0"

ui::
  # For content creators (Builder/Publisher mode)
  publisher::
    - ::
      type: "screen"
      id: "editor"
      blocks::
        - type: "input"
          # Interactive editing interface

  # For content viewers (Viewer mode)
  viewer::
    - ::
      type: "screen"
      id: "display"
      isEntryPoint: true
      blocks::
        - type: "text"
          # Read-only display interface
```

### Multi-Screen Navigation

Viewer mode supports multi-screen navigation:

```yaml
ui::
  viewer::
    - ::
      type: "screen"
      id: "home"
      isEntryPoint: true
      blocks::
        - type: "button"
          content: "Go to Page 2"
          action: "navigate"
          targetScreen: "page2"

    - ::
      type: "screen"
      id: "page2"
      blocks::
        - type: "button"
          content: "Back to Home"
          action: "navigate"
          targetScreen: "home"
```

---

## Component Cleanup

### Deleted Obsolete Components

The following components were identified as obsolete and removed:

1. **FolderManager.svelte** - Replaced by NavigationPanel
2. **ViewerMode.svelte** - Replaced by ViewerApp.svelte
3. **TemplateImportModal.svelte** - Unused (BuilderApp has inline import)

### Component Reuse

- **ViewerWebsiteFolder**: Viewer-specific variant with sync button
- **WebsiteFolder**: Used by Builder and Publisher modes
- **AddWebsiteConnectionModal**: Used by Viewer to connect to published sites
- **AddSovereignNodeModal**: Used by Builder/Publisher to add nodes

---

## Testing

### Test Template

**File**: `/docs/examples/17_viewer_navigation.huml`

A 3-screen test template demonstrating:
- Multi-screen navigation
- Button actions
- CSS styling
- Read-only content display

### Test Checklist

- [ ] NavigationPanel shows in all three modes
- [ ] "Add Sovereign Node" button in Builder/Publisher
- [ ] "Add Website" button in Viewer
- [ ] Panel collapse/expand works
- [ ] ModeSwitcher works from panel
- [ ] Viewer renders screens correctly
- [ ] Navigation between screens works
- [ ] CSS styling applies
- [ ] No console errors

---

## Future Enhancements (Out of Scope for Phases 1-4)

1. **Viewer Interactivity**: Support forms and submissions in viewer mode
2. **Real-time Updates**: Live sync of published content changes
3. **Offline Support**: Cache published content for offline viewing
4. **Sharing**: Generate shareable links for published content
5. **Analytics**: Track viewer engagement metrics

---

## References

- **Architecture Guide**: `/docs/architecture/STHALAM_ARCHITECTURE_GUIDE.md`
- **HUML Template Guide**: `/docs/specs/HUML_TEMPLATE_GUIDE_ACCURATE.md`
- **Examples**: `/docs/examples/`
- **BlockRenderer Design**: `/docs/architecture/NEW_BLOCK_RENDERER_DESIGN.md`

---

## Migration Notes

No breaking changes for existing templates. Templates without `ui.viewer` will show empty state in Viewer mode.

To add viewer support to existing templates:
1. Add `ui.viewer` section
2. Define viewer screens
3. Test navigation and rendering
