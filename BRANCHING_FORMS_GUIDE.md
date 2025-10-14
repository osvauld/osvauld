# Branching Forms with Viewport Navigation - Implementation Guide

## What Was Implemented

I've added a simple branching form system that uses viewport navigation instead of show/hide states. Here's what's new:

### New Block Types

1. **`branching-question`** - A yes/no question block
2. **`nav-button`** - A navigation button that moves viewport based on answers

### How It Works

1. User sees a yes/no question
2. User clicks Yes or No (selection is stored locally, not synced)
3. User clicks Next button
4. Viewport smoothly animates to different positions based on the answer
5. No state is synced to Yjs - only the answer selection is local

## Testing the Implementation

### Step 1: Create a Branching Question Block

In your builder mode, create a block with this structure:

```javascript
{
  id: "question-1",
  type: "branching-question",
  x: 100,
  y: 100,
  width: 600,
  height: 200,
  zIndex: 1,
  content: "",
  styles: {},
  question: "Are you a new user?",
  yesLabel: "Yes",
  noLabel: "No"
}
```

### Step 2: Create Content Sections

Create two different sections at different Y positions:

**Section for "Yes" answer:**
```javascript
{
  id: "section-yes",
  type: "text",
  x: 100,
  y: 1000,  // Far below the question
  width: 600,
  height: 300,
  zIndex: 1,
  content: "Welcome! Let's set up your account...",
  styles: {
    fontSize: "24px",
    fontWeight: "600"
  }
}
```

**Section for "No" answer:**
```javascript
{
  id: "section-no",
  type: "text",
  x: 800,
  y: 1000,  // Same Y as yes section, but to the right
  width: 600,
  height: 300,
  zIndex: 1,
  content: "Welcome back! What would you like to do?",
  styles: {
    fontSize: "24px",
    fontWeight: "600"
  }
}
```

### Step 3: Create Navigation Button

Create a nav button that links to the question:

```javascript
{
  id: "nav-btn-1",
  type: "nav-button",
  x: 350,
  y: 350,  // Below the question
  width: 200,
  height: 60,
  zIndex: 1,
  content: "Next",
  styles: {},
  questionId: "question-1",  // Links to the question above
  branches: {
    yes: { x: -100, y: -900, zoom: 1 },  // Viewport position for "yes" answer
    no: { x: -800, y: -900, zoom: 1 }    // Viewport position for "no" answer
  }
}
```

**Important:** The viewport coordinates are **relative to the current canvas transform**. The negative values move the viewport so that the content appears centered.

## How to Calculate Viewport Coordinates

To show a specific block centered in the viewport:

```
target_viewport_x = -(block.x - window_width/2)
target_viewport_y = -(block.y - window_height/2)
```

For example, if your window is 1200px wide and 800px tall:
- To center block at x=100, y=1000
- viewport_x = -(100 - 600) = 500
- viewport_y = -(1000 - 400) = -600

## Key Features

### Local State Only
- Answer selections are stored in component state
- Not synced to Yjs
- Each viewer has independent session

### Smooth Animation
- 500ms easing animation
- Prevents jarring viewport jumps
- Uses `easeInOutCubic` for natural motion

### Declarative Configuration
- No code shipping to viewer
- All logic defined in block properties
- Safe and predictable

### Visual Feedback
- Selected answer highlighted in blue
- Hover effects on buttons
- Disabled state when not in viewer mode

## Block Properties Reference

### `branching-question` Block
```typescript
{
  type: "branching-question",
  question: string,      // The question text
  yesLabel?: string,     // Label for yes button (default: "Yes")
  noLabel?: string,      // Label for no button (default: "No")
  // ... standard block properties
}
```

### `nav-button` Block
```typescript
{
  type: "nav-button",
  content: string,       // Button label (e.g., "Next")
  questionId: string,    // ID of the question block to read answer from
  branches: {
    yes: {
      x: number,         // Viewport X position for "yes" answer
      y: number,         // Viewport Y position for "yes" answer
      zoom?: number      // Optional zoom level (default: current zoom)
    },
    no: {
      x: number,         // Viewport X position for "no" answer
      y: number,         // Viewport Y position for "no" answer
      zoom?: number      // Optional zoom level (default: current zoom)
    }
  },
  // ... standard block properties
}
```

## Next Steps

To extend this implementation:

1. **Multiple Choice Questions**: Add more answer options beyond yes/no
2. **Back Button**: Store navigation history and allow going back
3. **Conditional Content**: Show/hide blocks based on answers without navigation
4. **Form Validation**: Require answer before allowing navigation
5. **Progress Indicator**: Show how far through the form the user is
6. **Skip Logic**: Allow skipping questions based on previous answers

## Styling

The new blocks come with default styling:
- Questions: Centered text with spaced buttons
- Nav buttons: Green with hover effects
- Selected answers: Blue highlight

You can customize by modifying the CSS in `Block.svelte`:
- `.block-branching-question`
- `.option-button`
- `.block-nav-button`

## Important Notes

1. **Readonly Mode Required**: Navigation only works in viewer mode (readonly=true)
2. **Answer Required**: Nav button warns if no answer selected (doesn't navigate)
3. **Animation Lock**: Prevents multiple simultaneous viewport animations
4. **No Yjs Sync**: Answer state is local only - intentional design choice

## Example Canvas Layout

```
┌─────────────────────────────────────┐
│  Question: "Are you a new user?"    │ (0, 0)
│  [ Yes ] [ No ]                     │
│  [ Next ]                           │
└─────────────────────────────────────┘
           │
           │ Click "Yes"
           ↓
┌─────────────────────────────────────┐
│  Welcome! Let's set up your         │ (0, 1000)
│  account...                         │
│  [form fields...]                   │
└─────────────────────────────────────┘

           OR

           Click "No"
           ↓
┌─────────────────────────────────────┐
│  Welcome back! What would you       │ (1500, 1000)
│  like to do?                        │
│  [different content...]             │
└─────────────────────────────────────┘
```

The viewport smoothly animates from (0,0) to either (0, 1000) or (1500, 1000) based on the answer!
