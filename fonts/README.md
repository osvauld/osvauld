# @osvauld/fonts

Self-hosted fonts package for the Osvauld monorepo. This package provides Inter, Plus Jakarta Sans, and Host Grotesk fonts as CSS files that can be imported across all projects in the monorepo.

## Available Fonts

- **Inter** - Modern sans-serif variable font with full weight range (100-900)
- **Plus Jakarta Sans** - Friendly sans-serif variable font with weight range (200-800)
- **Host Grotesk** - Geometric sans-serif font with weights: 200, 300, 400, 500, 600, 700

## Font Technology

Inter and Plus Jakarta Sans use **variable fonts** for optimal performance and rendering quality:
- Single file for all weights (instead of 6+ separate files)
- Smooth weight interpolation
- Better rendering consistency across browsers
- Matches Google Fonts rendering exactly
- Automatic fallback to static fonts for older browsers

## Usage

### In HTML files
```html
<!-- Import specific font families (use package path, not node_modules) -->
<link rel="stylesheet" href="@osvauld/fonts/src/fonts/inter.css" />
<link rel="stylesheet" href="@osvauld/fonts/src/fonts/plus-jakarta-sans.css" />
<link rel="stylesheet" href="@osvauld/fonts/src/fonts/host-grotesk.css" />
```

### In CSS files
```css
@import '@osvauld/fonts/src/fonts/inter.css';
@import '@osvauld/fonts/src/fonts/plus-jakarta-sans.css';
```

### In TypeScript/JavaScript
```typescript
import '@osvauld/fonts/src/fonts/inter.css';
import { fontFamilies } from '@osvauld/fonts';
```

## Font Files Structure

```
fonts/
├── src/
│   ├── fonts/
│   │   ├── inter/
│   │   │   ├── InterVariable.woff2 (variable font - all weights)
│   │   │   └── InterVariable-Italic.woff2 (italic variable font)
│   │   ├── plus-jakarta-sans/
│   │   │   ├── PlusJakartaSans-Variable.woff2 (variable font - all weights)
│   │   │   └── PlusJakartaSans-Italic-Variable.woff2 (italic variable font)
│   │   └── host-grotesk/
│   │       └── HostGrotesk-*.ttf (static fonts)
│   └── index.ts
└── package.json
```

## Performance Benefits

**Variable Fonts (Inter & Plus Jakarta Sans):**
- **File Size**: ~337KB (Inter) vs ~2MB for 6 static fonts
- **HTTP Requests**: 1 file vs 6 files  
- **Rendering**: Matches Google Fonts exactly
- **Browser Support**: 95%+ (older browsers fall back to system fonts)
- **Clean Implementation**: No unused static font files

## Adding New Fonts

To add new font files:

1. Place font files in the appropriate directory under `src/fonts/`
2. For variable fonts, use WOFF2 format for best performance
3. Include static TTF fallbacks for older browsers
4. Update the corresponding CSS file with @font-face declarations

## CSS Classes

The fonts are available with their standard font-family names:
- `font-family: 'Inter'`
- `font-family: 'Plus Jakarta Sans'`  
- `font-family: 'Host Grotesk'`
