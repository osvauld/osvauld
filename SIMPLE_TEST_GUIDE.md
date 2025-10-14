# Simple Test Guide - Branching Forms

## Quick Start

### 1. Start the App

```bash
cd /home/abe/osvauld/sthalam
cargo tauri dev
```

### 2. Create the Test Content (Builder Mode)

1. **Create/select a Website resource**
2. **Add blocks from the Block Palette** (left sidebar):
   - Add **"Yes/No Question"**
   - Add **"Nav Button"**
   - Add **two "Text"** blocks

3. **Position the blocks**:
   ```
   At (100, 100):    Yes/No Question - "Are you a new user?"
   At (350, 350):    Nav Button - "Next"
   At (100, 1000):   Text - "Welcome! Setup..."  (for Yes answer)
   At (1500, 1000):  Text - "Welcome back!"      (for No answer)
   ```

4. **Configure the Nav Button** (via browser console):

Open DevTools (F12) and run:

```javascript
// Auto-configure the branching form
const coordinator = dataState?.getBlocksuiteCoordinator?.();
if (coordinator) {
  const docs = coordinator.getDocuments();
  const blocks = docs.blocks;

  // Find blocks
  let questionId, navBtnId;
  blocks.forEach((block, id) => {
    if (block.type === 'branching-question') questionId = id;
    if (block.type === 'nav-button') navBtnId = id;
  });

  // Link nav button to question
  if (questionId && navBtnId) {
    const navBtn = blocks.get(navBtnId);
    blocks.set(navBtnId, {
      ...navBtn,
      questionId: questionId,
      branches: {
        yes: { x: -100, y: -900, zoom: 1 },
        no: { x: -1500, y: -900, zoom: 1 }
      }
    });
    console.log("✅ Configured! Question:", questionId, "NavBtn:", navBtnId);
  }
}
```

### 3. Test in Builder Mode (Quick Test)

You can actually test it right in builder mode by temporarily making the canvas readonly:

**In browser console:**

```javascript
// Quick test without switching modes
// This will make blocks interactive temporarily

// Store original blocks
const canvas = document.querySelector('[class*="canvas"]');

// You'll need to manually click and test
// Or switch to actual Viewer Mode (next step)
```

### 4. Test in Viewer Mode (Proper Test)

**Option A: If your app has a mode toggle**
- Look for "Viewer Mode" button/toggle in the UI
- Click it to switch modes

**Option B: Navigate to Viewer Mode manually**
- Your app might have separate routes for Builder vs Viewer
- Check if there's a navigation option to "View" or "Preview"

**Option C: Test via console (if Canvas is rendered with readonly=true somewhere)**

The ViewerMode.svelte component renders Canvas with `readonly={true}`, which enables interactions.

### 5. Interact with the Form

In viewer mode (or with readonly=true):

1. **Click "Yes"** → Button turns blue
2. **Click "Next"** → Viewport smoothly moves down to show "Welcome! Setup..." text
3. Pan back up to the question
4. **Click "No"** → Button turns blue
5. **Click "Next"** → Viewport moves down-right to show "Welcome back!" text

## Expected Behavior

✅ Answer selection highlights in blue
✅ Smooth 500ms animation when clicking Next
✅ Viewport moves to show appropriate content based on answer
✅ Console logs show navigation flow

## Console Output (when working)

```
🧭 Navigation triggered by button: block-123456
📋 User answered: yes
🎯 Navigating to: {x: -100, y: -900, zoom: 1}
✅ Navigation complete
```

## Troubleshooting

**"No answer selected" warning**
- You need to click Yes or No before clicking Next

**Navigation button doesn't work**
- Make sure you're in viewer mode (readonly=true)
- Check console for the configuration step - question ID must be set

**Viewport doesn't move**
- Check that branch coordinates are configured
- Verify content blocks are positioned around Y=1000

**Can't find viewer mode**
- You might need to "publish" the resource first
- Or check your app's navigation for a preview/view option

## Understanding the Coordinates

The viewport coordinates are **inverted** because they represent the canvas offset:

- Content at Y=1000 → Viewport Y=-900 (to bring it into view)
- Content at X=1500 → Viewport X=-1400 (to center it)

The viewport moves the canvas up/left (negative values) to show content that's positioned down/right (positive values).

## What's Next?

Once this works, you can:
- Add more branching paths
- Create multi-step flows
- Add forms, images, or other content at each branch
- Experiment with different layouts

The key insight: **Everything is spatial.** You're navigating a 2D space, not showing/hiding content.
