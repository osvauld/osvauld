// Font exports for @osvauld/fonts package
// This allows easy importing of font CSS files across the monorepo

// Individual font family CSS files
export { default as InterCSS } from './fonts/inter.css';
export { default as PlusJakartaSansCSS } from './fonts/plus-jakarta-sans.css';
export { default as HostGroteskCSS } from './fonts/host-grotesk.css';

// Re-export CSS file paths for direct imports
export const fontPaths = {
  inter: './fonts/inter.css',
  plusJakartaSans: './fonts/plus-jakarta-sans.css',
  hostGrotesk: './fonts/host-grotesk.css'
} as const;

// Font family names for easy reference
export const fontFamilies = {
  inter: 'Inter',
  plusJakartaSans: 'Plus Jakarta Sans',
  hostGrotesk: 'Host Grotesk'
} as const;
