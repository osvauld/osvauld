# How to Test Branching Forms

## Step-by-Step Testing Guide

### Step 1: Start the Application

```bash
cd /home/abe/osvauld/sthalam
cargo tauri dev
```

Wait for the application to compile and launch.

### Step 2: Create or Select a Website Resource

1. In the navigation panel (left sidebar), create a new "Website" resource or select an existing one
2. Make sure you're in **Builder Mode** (this is the default editing mode)

### Step 3: Add the Blocks

You'll create a simple branching form with 4 components:

#### A. Add the Question Block

1. In the **Block Palette** (left side), scroll down and click **"Yes/No Question"** (icon: ?)
2. A question block will appear in the center of your canvas
3. It will say "Are you a new user?" with Yes/No buttons (these are disabled in builder mode)

#### B. Add Content for "Yes" Answer

1. Pan the canvas DOWN (click and drag the background down) so you can see empty space below the question
2. Add a **"Text"** block from the palette
3. Position it at approximately **Y = 1000** (far below the question)
   - You can drag it down with Ctrl+Click and drag
4. Click on this text block and edit it to say: **"Welcome! Let's set up your account..."**
5. Make note of this block's ID (you can see it in the properties panel on the right)

#### C. Add Content for "No" Answer

1. Pan the canvas to the RIGHT and down
2. Add another **"Text"** block
3. Position it at approximately **X = 1500, Y = 1000** (to the right of the "yes" content)
4. Edit it to say: **"Welcome back! What would you like to do?"**

#### D. Add the Navigation Button

1. Pan back to the original question area
2. Add a **"Nav Button"** from the palette (icon: ➜)
3. Position it **below the question** (around Y = 350)
4. It will show "Next" text

### Step 4: Configure the Navigation Button

This is the crucial step - linking the button to the question and setting up viewport coordinates.

1. **Select the Nav Button** (click on it)
2. **Open Properties Panel** (right side of screen)
3. You'll need to manually configure the `questionId` and `branches` properties

Unfortunately, the Properties Panel might not have UI for these yet, so you'll need to do this via browser console:

**Open Browser DevTools** (F12 or Ctrl+Shift+I), then paste this in the console:

```javascript
// First, get the question block ID
// Look at the first branching-question block in the canvas
const questionBlock = Array.from(document.querySelectorAll('[class*="block-branching-question"]'))[0];
const questionId = questionBlock?.closest('[class*="block"]')?.style.cssText.match(/--block-id:\s*([^;]+)/)?.[1];

console.log("Question ID:", questionId); // Copy this ID

// Now update the nav button
// Find the nav button and get its ID
const navButton = Array.from(document.querySelectorAll('[class*="block-nav-button"]'))[0];
// You'll need to update via Yjs - this is a workaround
```

**Easier Method: Edit Properties Panel**

Or wait - let me check if there's a simpler way. Actually, let's use the block IDs directly:

1. Select the question block, note its ID from properties panel (e.g., `block-1234567890`)
2. Select the nav button
3. In the properties panel, if you see `questionId`, set it to the question's ID
4. For the branches, you need to set viewport coordinates

### Step 5: Quick Test with Console

Since properties panel might not support all fields yet, here's a console command to set everything up:

**In Browser Console (F12):**

```javascript
// Get the coordinator from dataState
const coordinator = window.__dataState?.getBlocksuiteCoordinator?.();
if (!coordinator) {
  console.error("Coordinator not found");
} else {
  const docs = coordinator.getDocuments();
  const blocks = docs.blocks;

  // Find the question block
  let questionId = null;
  let navButtonId = null;

  blocks.forEach((block, id) => {
    if (block.type === 'branching-question') {
      questionId = id;
      console.log("Found question:", id);
    }
    if (block.type === 'nav-button') {
      navButtonId = id;
      console.log("Found nav button:", id);
    }
  });

  if (questionId && navButtonId) {
    // Get the nav button
    const navButton = blocks.get(navButtonId);

    // Update it with proper configuration
    blocks.set(navButtonId, {
      ...navButton,
      questionId: questionId,
      branches: {
        yes: { x: -100, y: -900, zoom: 1 },
        no: { x: -1500, y: -900, zoom: 1 }
      }
    });

    console.log("✅ Nav button configured!");
    console.log("Question ID:", questionId);
    console.log("Nav button updated with branches");
  } else {
    console.error("Missing blocks. Add both a branching-question and nav-button first.");
  }
}
```

### Step 6: Switch to Viewer Mode

Now test the interaction:

1. Look for a **"Viewer Mode"** toggle or button in your UI
2. Or check if there's a way to preview in readonly mode
3. If ViewerMode.svelte is a separate component, you might need to navigate to it

**If you have a viewer mode toggle:**
- Click it to enable viewer mode
- The blocks should now be interactive

**If not, you can test with console:**

```javascript
// Force readonly mode on canvas (temporary test)
// This makes the blocks interactive
const canvasComponent = document.querySelector('[class*="canvas"]');
// The Canvas component needs readonly=true prop
```

### Step 7: Test the Interaction

Once in viewer mode:

1. **Click "Yes"** - the button should highlight in blue
2. **Click "Next"** - the viewport should smoothly animate DOWN to show the "Welcome! Let's set up..." text
3. **Pan back to the question** (drag the canvas)
4. **Click "No"** - that button should highlight
5. **Click "Next"** - the viewport should smoothly animate DOWN and RIGHT to show the "Welcome back..." text

### Step 8: Observe the Animation

You should see:
- ✅ Smooth 500ms animation
- ✅ Viewport moves to show the appropriate content
- ✅ Console logs showing the navigation flow

## Troubleshooting

### "No answer selected" Warning

If you click Next without selecting Yes/No, you'll see a console warning. This is expected - select an answer first.

### Navigation Button Doesn't Work

Check console (F12) for errors:
- Make sure `questionId` is set correctly
- Make sure you're in viewer mode (readonly=true)
- Check that branches are configured with x, y coordinates

### Viewport Doesn't Move

- Check browser console for the "Navigating to:" log
- Verify the coordinates are negative (viewport coordinates are inverted)
- Make sure content blocks are positioned where you expect (Y around 1000)

### Content Not Visible After Navigation

- The viewport might be at the wrong coordinates
- Try adjusting the branch coordinates:
  - For "yes" content at (100, 1000): viewport should be around (0, -900)
  - For "no" content at (1500, 1000): viewport should be around (-1400, -900)

## Expected Console Output

When clicking Next after selecting Yes:

```
🧭 Navigation triggered by button: block-XXXXXX
📋 User answered: yes
🎯 Navigating to: {x: -100, y: -900, zoom: 1}
✅ Navigation complete
```

## Next Steps After Testing

Once basic navigation works, you can:

1. Add more content sections at different positions
2. Create multi-step branching (question → answer → another question → etc.)
3. Add images, forms, or other blocks at each branch location
4. Experiment with different layouts (tree, grid, parallel paths)

## Notes

- Answers are stored **locally only** - not synced to Yjs
- Each viewer has independent state
- Refreshing the page resets answer state (starts from beginning)
- Builder mode (readonly=false) disables interactions - only viewer mode works
