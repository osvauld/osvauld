/// <reference types="vite/client" />

// Declare CSS imports
declare module '*.css' {
  const content: string;
  export default content;
}

// Declare custom elements
declare namespace JSX {
  interface IntrinsicElements {
    'affine-editor-container': any;
    'editor-host': any;
  }
}
