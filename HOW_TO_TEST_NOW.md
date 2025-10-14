# How to Test Branching Forms - With UI Configuration!

## What Changed

You now have a **Properties Panel UI** to configure branching navigation! No more console commands - just dropdowns.

## Step-by-Step Test

### 1. Start the App

```bash
cd /home/abe/osvauld/sthalam/src-tauri
cargo tauri dev
```

### 2. Create Your Content

**In Builder Mode:**

1. **Create/select a Website resource**

2. **Add these blocks** from Block Palette (left sidebar):
   - **"Yes/No Question"** - appears in center
   - **"Nav Button"** - drag it below the question
   - **Two "Text" blocks** - these will be your answer destinations

3. **Position your content:**
   - Keep question + nav button at the top (around Y = 0-400)
   - **Pan canvas DOWN** (drag background upward) to go below
   - Position first text block at **Y = 1000** (left side) - edit to say "You chose YES!"
   - Position second text block at **Y = 1000** (right side, X = 1500) - edit to say "You chose NO!"

### 3. Configure the Question

1. **Click on the Yes/No Question block**
2. **In Properties Panel (right side)**, you'll see "Question Settings":
   - Change question text if you want
   - Change Yes/No labels if you want

### 4. Configure the Navigation Button

1. **Click on the Nav Button block**
2. **In Properties Panel**, you'll see "Navigation Settings":

   **Step 1: Link to Question**
   - Click the "Link to Question" dropdown
   - Select your question from the list

   **Step 2: Configure YES path**
   - Click "When 'Yes' → Navigate to Block" dropdown
   - Select the "text at (100, 1000)" block (your YES content)
   - ✓ You'll see "YES path configured" with viewport coordinates

   **Step 3: Configure NO path**
   - Click "When 'No' → Navigate to Block" dropdown
   - Select the "text at (1500, 1000)" block (your NO content)
   - ✓ You'll see "NO path configured" with viewport coordinates

**That's it! The viewport coordinates are calculated automatically!**

### 5. Test in Viewer Mode

Now you need to view it in viewer mode (readonly mode):

**Option A: If your app has a Preview/View button**
- Click it to switch to viewer mode

**Option B: If you need to publish first**
- You might need to "publish" the resource
- Then navigate to it in viewer mode

**Option C: Check if there's a viewer route in your app**
- Your app has ViewerMode.svelte component
- Navigate to viewer section

### 6. Interact with the Form

Once in viewer mode:

1. **Click "Yes"** → Button highlights blue
2. **Click "Next"** → Viewport smoothly animates down to show "You chose YES!" text
3. **Pan back up** to the question (drag canvas)
4. **Click "No"** → Button highlights blue
5. **Click "Next"** → Viewport animates to show "You chose NO!" text

## What You Should See

✅ Question displays with Yes/No buttons
✅ Clicking answer highlights the button in blue
✅ Clicking Next triggers smooth 500ms animation
✅ Viewport moves to show the appropriate content block
✅ No console commands needed!

## Key Points

- **Viewport coordinates are automatic** - just select which block to show
- **The system calculates** the viewport position to center that block
- **No manual coordinate entry** - the dropdowns handle everything
- **Visual feedback** in properties panel shows configured paths

## The Coordinate Calculation

When you select a block in the dropdown, the system:
1. Gets the block's position (x, y)
2. Calculates: `viewportX = -(blockX - 600)` (assumes ~1200px wide window)
3. Calculates: `viewportY = -(blockY - 400)` (assumes ~800px tall window)
4. Stores these in the nav button's `branches` property
5. Shows you the coordinates in the preview

The negative values are because viewport offset is inverted - moving viewport UP (negative Y) shows content that's DOWN (positive Y).

## Troubleshooting

**Don't see the dropdowns?**
- Make sure you selected the Nav Button block
- Properties panel should show "Navigation Settings" section

**Dropdown is empty for "Link to Question"?**
- You need to add a "Yes/No Question" block first
- The dropdown only shows branching-question type blocks

**Dropdown is empty for targets?**
- You need to add some content blocks (text, image, etc.)
- The dropdown shows all blocks with their positions

**Navigation doesn't work?**
- Make sure you're in **viewer mode** (readonly=true)
- In builder mode, interactions are disabled

**Viewport doesn't move to the right spot?**
- The calculation assumes ~1200x800 window size
- You can manually adjust coordinates if needed (shown in preview)

## What's Next?

Once this works:
- Add more questions and create multi-step flows
- Add images, forms, or other rich content at each branch
- Create complex decision trees
- Test different layouts (tree, grid, parallel paths)

The key insight: **You're navigating spatial positions on an infinite canvas**, not showing/hiding content. Everything exists, you just move the camera to look at different parts!
