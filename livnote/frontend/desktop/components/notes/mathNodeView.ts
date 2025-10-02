/**
 * This file is kept for future customization if needed.
 * 
 * The prosemirror-math library provides its own NodeView that handles:
 * - KaTeX rendering
 * - Edit/display mode toggle
 * - Live preview while editing
 * 
 * Our equation numbering is handled via:
 * 1. Custom attributes (label, number) in the node spec
 * 2. The mathNumberingPlugin that computes and updates numbers
 * 3. CSS that displays the numbers visually
 * 
 * If you need to customize the NodeView behavior beyond what the library provides,
 * you can extend the library's NodeView class here and pass it to mathPlugin.
 * 
 * For now, we use the library's default NodeView as-is.
 */

export { }; // Make this a module
