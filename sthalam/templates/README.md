# Sthalam Templates

This directory contains HUML templates for quickly building websites and applications in Sthalam.

## What is HUML?

HUML (Human-oriented Markup Language) is a clean, readable format for defining structured data. It's like YAML but with stricter formatting rules and better support for multi-line content.

## How to Use Templates

1. **Open Sthalam** and create or select a resource
2. **Click the "📥 Import Template" button** in the Block Palette (left sidebar)
3. **Paste the HUML template** from one of the files below
4. **Click "Import Template"**
5. **Done!** Your website is automatically generated

## Available Templates

### Dev Confession Blog

A fun, developer-themed blog showcasing Sthalam's capabilities.

**Files:**
- `dev-blog-basic.huml` - Simple structure with minimal styling
- `dev-blog-enhanced.huml` - Full Catppuccin theme with advanced CSS

**Features:**
- Multi-screen navigation
- Thread blocks with collaborative comments
- Responsive card layout
- Navigation buttons between posts
- Catppuccin color scheme

**Screens:**
1. Home - Blog feed with 3 post cards
2. Post 1 - "I Use Light Mode"
3. Post 2 - "I Don't Know What DNS Is"
4. Post 3 - "My Code Works and I Don't Know Why"

**Best for:** Learning the basics, understanding screen navigation, testing thread blocks

---

## HUML Syntax Quick Reference

### Basic Structure

```huml
name: "My Website"
resourceType: "website"

screens::
  - id: "home"
    name: "Home"
    isEntryPoint: true
    children::
      - type: "heading"
        content: "Hello World"
```

### Block Types

- `screen-container` - Main responsive page container
- `section-container` - Layout section (supports flexbox/grid)
- `heading` - Large heading text
- `text` - Paragraph text
- `markdown-text` - Rich text with markdown
- `image` - Image or GIF
- `thread` - Blog post with collaborative comments
- `form` - Form metadata (invisible)
- `form-field-text` - Text input
- `form-field-textarea` - Multi-line text
- `form-field-checkbox` - Checkbox
- `form-submit-button` - Submit button
- `nav-button` - Navigation/show/hide button
- `branching-question` - Yes/No conditional navigation

### Multi-line Content

Use triple quotes for CSS and markdown:

```huml
css: """
  display: flex;
  gap: 20px;
  padding: 40px;
"""

content: """
# Markdown Title

Paragraph text here...
"""
```

### Nested Children

Use `children::` with indented list:

```huml
- type: "section-container"
  children::
    - type: "heading"
      content: "Title"

    - type: "text"
      content: "Description"
```

### Navigation

```huml
- type: "nav-button"
  content: "Go to About"
  action: "navigate"
  targetScreen: "about"
```

## Creating Your Own Templates

1. Build your website normally in Sthalam
2. **Export structure** (coming soon)
3. Save as `.huml` file
4. Share with others!

## Tips

- Start with a basic template and customize
- Use the enhanced versions to learn advanced CSS
- Test import on a new resource first
- Check the browser console if import fails

## Catppuccin Colors

The templates use the Catppuccin color palette:

- Background: `#010409` (ninjablack)
- Cards: `#0d0e13` (frameblack)
- Borders: `#30363d`
- Primary: `#89b4fa` (carolinablue)
- Accent: `#cba6f7` (lilacpink)
- Success: `#a6e3a1` (grapegreen)
- Text: `#c9d1d9` (quarzowhite)
- Muted: `#6e7681` (sheffieldgrey)

## Troubleshooting

**Import fails:**
- Check HUML syntax (indentation must be 2 spaces)
- Ensure all required fields are present
- Check browser console for errors

**Blocks don't render:**
- Make sure you have a screen with `isEntryPoint: true`
- Verify block types are correct
- Check that `targetScreen` IDs exist

**Navigation doesn't work:**
- Ensure `targetScreen` matches a screen `id`
- Check that screens have unique IDs

## More Templates Coming Soon

- Pizza Quest (branching form game)
- Cat Facts Landing Page (CSS showcase)
- Meme Forum (multiple threads)
- Cat vs Dog Quiz (advanced branching)

---

Built with ❤️ for the Sthalam community
