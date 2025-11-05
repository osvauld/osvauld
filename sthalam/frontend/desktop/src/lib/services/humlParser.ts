/**
 * HUML Parser TypeScript Wrapper
 *
 * Loads the WASM-based HUML parser and provides a clean API for parsing
 * HUML template files into JavaScript objects.
 */

interface HUMLParserResult {
  success: boolean;
  result: any;
  error: string | null;
}

interface HUMLParser {
  parse(humlString: string): HUMLParserResult;
  version: string;
}

let humlParser: HUMLParser | null = null;

/**
 * Load the HUML parser WASM module
 * Must be called before using parseHUML()
 */
export async function loadHUMLParser(): Promise<void> {
  if (humlParser) return;

  return new Promise((resolve, reject) => {
    const script = document.createElement('script');

    // Detect Tauri environment
    const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;

    // Use correct path based on environment
    script.src = isTauri
      ? 'asset://localhost/huml_parser.js'
      : '/huml_parser.js';

    script.onload = () => {
      // WASM needs time to initialize
      const checkInterval = setInterval(() => {
        if (typeof (window as any).HUMLParser !== 'undefined') {
          clearInterval(checkInterval);
          humlParser = (window as any).HUMLParser;
          console.log('[HUML Parser] Loaded successfully, version:', humlParser?.version);
          resolve();
        }
      }, 100);

      // Timeout after 10 seconds
      setTimeout(() => {
        clearInterval(checkInterval);
        reject(new Error('Timeout waiting for HUMLParser to initialize'));
      }, 10000);
    };

    script.onerror = () => {
      reject(new Error('Failed to load HUML parser script'));
    };

    document.head.appendChild(script);
  });
}

/**
 * Parse a HUML string into a JavaScript object
 *
 * @param humlString - The HUML template string to parse
 * @returns The parsed JavaScript object
 * @throws Error if parsing fails
 */
export function parseHUML(humlString: string): any {
  if (!humlParser) {
    throw new Error('HUML parser not loaded. Call loadHUMLParser() first.');
  }

  const result = humlParser.parse(humlString);

  if (!result.success) {
    throw new Error(`HUML parsing failed: ${result.error}`);
  }

  return result.result;
}

/**
 * Parse a HUML string and return result without throwing
 *
 * @param humlString - The HUML template string to parse
 * @returns Parse result with success flag
 */
export function tryParseHUML(humlString: string): HUMLParserResult {
  if (!humlParser) {
    return {
      success: false,
      result: null,
      error: 'HUML parser not loaded. Call loadHUMLParser() first.'
    };
  }

  return humlParser.parse(humlString);
}

/**
 * Get the supported HUML version
 */
export function getHUMLVersion(): string {
  if (!humlParser) {
    throw new Error('HUML parser not loaded. Call loadHUMLParser() first.');
  }
  return humlParser.version;
}

/**
 * Check if the HUML parser is loaded
 */
export function isHUMLParserLoaded(): boolean {
  return humlParser !== null;
}
