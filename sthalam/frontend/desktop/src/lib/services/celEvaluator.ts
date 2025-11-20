/**
 * CEL Evaluator Service
 *
 * TypeScript wrapper for the OCaml/WASM CEL evaluator.
 * Provides direct JavaScript object access for lazy evaluation.
 */

/**
 * WASM CEL Evaluator interface
 */
interface CELEvaluator {
  /**
   * Evaluate a CEL expression with context
   * @param expr CEL expression string
   * @param context JavaScript object (Loro map or plain object)
   * @returns Result object
   */
  evaluate(
    expr: string,
    context: any
  ): {
    success: boolean;
    result?: any;
    error?: string;
  };

  /**
   * Interpolate CEL expressions in template string
   * Replaces {{ expr }} with evaluated results
   * @param template Template string with {{ }} markers
   * @param context JavaScript object
   * @returns Interpolated string
   */
  interpolate(template: string, context: any): string;

  /**
   * Generate unique asset ID
   * @param assetType "video", "image", or "file"
   * @returns Unique asset ID string
   */
  generateAssetId(assetType: string): string;

  /**
   * Evaluate expression for grid (canvas rendering)
   * @param expr CEL expression string
   * @param gridSize Grid size (e.g., 100 for 100x100)
   * @param time Current time for animation
   * @returns Result with Uint8Array output
   */
  evaluateGrid(
    expr: string,
    gridSize: number,
    time: number
  ): {
    success: boolean;
    output?: Uint8Array;
    error?: string;
  };

  /**
   * Get evaluator version
   */
  version(): string;

  /**
   * Get list of supported functions
   */
  supportedFunctions(): string[];
}

/**
 * WASM CEL Reactive interface (OCaml React FRP)
 */
interface CELReactive {
  /**
   * Initialize reactive system from HUML template
   * Parses publisherState and publisherComputed to create signal graph
   * @param template Parsed HUML template object
   */
  initTemplate(template: any): void;

  /**
   * Update a reactive state signal (triggers reactive propagation)
   * @param name Signal name
   * @param value New value
   */
  updateState(name: string, value: any): void;

  /**
   * Get current value of a reactive signal
   * @param name Signal name
   * @returns Current value or undefined
   */
  getValue(name: string): any;

  /**
   * Get all current reactive signal values
   * @returns Object with all signal values
   */
  getAllValues(): Record<string, any>;

  /**
   * Set log level for WASM logging
   * @param level Log level (e.g., "debug", "info", "warn", "error")
   */
  setLogLevel(level: string): void;

  /**
   * Get reactive system version
   */
  version: string;
}

/**
 * Global CEL evaluator instance (set by loadCELEvaluator)
 */
let celEvaluator: CELEvaluator | null = null;

/**
 * Global CEL reactive instance (set by loadCELEvaluator)
 */
let celReactive: CELReactive | null = null;

/**
 * Load the CEL evaluator WASM module
 * @returns Promise that resolves when evaluator is loaded
 */
export async function loadCELEvaluator(): Promise<void> {
  if (celEvaluator) {
    return; // Already loaded
  }

  return new Promise((resolve, reject) => {
    const script = document.createElement('script');

    // Detect Tauri environment
    const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;

    // Use asset:// protocol in Tauri, regular path in browser
    script.src = isTauri
      ? 'asset://localhost/cel_eval.js'
      : '/cel_eval.js';

    script.onload = () => {
      // WASM needs time to initialize
      const checkInterval = setInterval(() => {
        if (typeof (window as any).CELEvaluator !== 'undefined' &&
            typeof (window as any).CELReactive !== 'undefined') {
          clearInterval(checkInterval);
          celEvaluator = (window as any).CELEvaluator;
          celReactive = (window as any).CELReactive;
          console.log('[CEL] Evaluator loaded, version:', celEvaluator!.version());
          console.log('[CEL] Reactive loaded, version:', celReactive!.version);
          resolve();
        }
      }, 100);

      // Timeout after 10 seconds
      setTimeout(() => {
        clearInterval(checkInterval);
        reject(new Error('Timeout waiting for CELEvaluator/CELReactive to initialize'));
      }, 10000);
    };

    script.onerror = (error) => {
      reject(new Error(`Failed to load CEL evaluator: ${error}`));
    };

    document.head.appendChild(script);
  });
}

/**
 * Ensure CEL evaluator is loaded
 * @throws Error if evaluator not loaded
 */
function ensureLoaded(): CELEvaluator {
  if (!celEvaluator) {
    throw new Error('CEL evaluator not loaded. Call loadCELEvaluator() first.');
  }
  return celEvaluator;
}

/**
 * Evaluate a CEL expression with context
 *
 * Uses direct JavaScript object access - no JSON serialization!
 * Supports Loro maps, plain objects, and any JavaScript value.
 *
 * **NOTE on ${ } syntax:**
 * The OCaml WASM layer automatically strips `${ }` markers before parsing.
 * Blocks should pass expressions as-is without checking for `${ }`.
 * Both `"${ user.name }"` and `"user.name"` work identically.
 *
 * @param expr CEL expression string (with or without ${ } markers)
 * @param context JavaScript object (Loro map, plain object, etc.)
 * @returns Evaluation result
 *
 * @example
 * ```typescript
 * const loroMap = loro.getMap("appState");
 * const result = evaluateCEL("user.name", loroMap);
 * console.log(result); // "Alice"
 * ```
 *
 * @example
 * ```typescript
 * const context = { count: 5, items: [1, 2, 3] };
 * const result = evaluateCEL("count * items.size()", context);
 * console.log(result); // 15
 * ```
 *
 * @example
 * ```typescript
 * // Both syntaxes work identically:
 * evaluateCEL("${ user.name }", context);  // Strips ${ } → evaluates "user.name"
 * evaluateCEL("user.name", context);       // Evaluates "user.name" directly
 * ```
 */
export function evaluateCEL(expr: string, context: any): any {
  const evaluator = ensureLoaded();

  const result = evaluator.evaluate(expr, context);

  if (!result.success) {
    throw new Error(`CEL evaluation failed: ${result.error}`);
  }

  return result.result;
}

/**
 * Interpolate CEL expressions in a template string
 *
 * Replaces all {{ expr }} patterns with evaluated results.
 *
 * @param template Template string with {{ }} markers
 * @param context JavaScript object
 * @returns Interpolated string
 *
 * @example
 * ```typescript
 * const template = "Hello, {{ user.name }}! You have {{ posts.size() }} posts.";
 * const context = { user: { name: "Alice" }, posts: [1, 2, 3] };
 * const result = interpolateCEL(template, context);
 * console.log(result); // "Hello, Alice! You have 3 posts."
 * ```
 */
export function interpolateCEL(template: string, context: any): string {
  const evaluator = ensureLoaded();
  return evaluator.interpolate(template, context);
}

/**
 * Evaluate expression for grid (canvas pattern rendering)
 *
 * Highly optimized for 10,000+ evaluations per frame.
 * Returns Uint8Array for zero-copy GPU upload.
 *
 * @param expr CEL expression string (can use x, y, time, gridSize variables)
 * @param gridSize Grid size (100 means 100x100 = 10,000 cells)
 * @param time Current time for animation
 * @returns Uint8Array with brightness values (0-255)
 *
 * @example
 * ```typescript
 * const expr = "sin(x * 0.2 + time) + cos(y * 0.2 + time)";
 * const result = evaluateGridCEL(expr, 100, performance.now() / 1000);
 * if (result) {
 *   // Upload directly to WebGL
 *   gl.texImage2D(gl.TEXTURE_2D, 0, gl.LUMINANCE,
 *                 100, 100, 0, gl.LUMINANCE, gl.UNSIGNED_BYTE, result);
 * }
 * ```
 */
export function evaluateGridCEL(
  expr: string,
  gridSize: number,
  time: number
): Uint8Array | null {
  const evaluator = ensureLoaded();

  const result = evaluator.evaluateGrid(expr, gridSize, time);

  if (!result.success) {
    console.error('[CEL] Grid evaluation failed:', result.error);
    return null;
  }

  return result.output || null;
}

/**
 * Get CEL evaluator version
 */
export function getCELVersion(): string {
  const evaluator = ensureLoaded();
  return evaluator.version();
}

/**
 * Get list of supported CEL functions
 */
export function getSupportedFunctions(): string[] {
  const evaluator = ensureLoaded();
  return evaluator.supportedFunctions();
}

/**
 * Check if CEL evaluator is loaded
 */
export function isCELLoaded(): boolean {
  return celEvaluator !== null;
}

/**
 * Generate unique asset ID using OCaml WASM module
 *
 * Format: asset_{type}_{timestamp}_{random}
 *
 * @param assetType "video", "image", or "file"
 * @returns Unique asset ID string
 *
 * @example
 * ```typescript
 * const videoId = generateAssetIdCEL("video");
 * console.log(videoId); // "asset_video_1762338443248_a1ut8oak4"
 *
 * const imageId = generateAssetIdCEL("image");
 * console.log(imageId); // "asset_image_1762338443250_b2vt9pblk5"
 * ```
 */
export function generateAssetIdCEL(assetType: 'video' | 'image' | 'file' | 'audio'): string {
  const evaluator = ensureLoaded();
  return evaluator.generateAssetId(assetType);
}

/**
 * ==== REACTIVE SYSTEM API ====
 */

/**
 * Ensure CEL reactive is loaded
 * @throws Error if reactive not loaded
 */
function ensureReactiveLoaded(): CELReactive {
  if (!celReactive) {
    throw new Error('CEL reactive not loaded. Call loadCELEvaluator() first.');
  }
  return celReactive;
}

/**
 * Initialize reactive system from HUML template
 * Parses publisherState and publisherComputed to create signal graph
 *
 * @param template Parsed HUML template object (must have documents.publisherState and documents.publisherComputed)
 *
 * @example
 * ```typescript
 * const template = parseHUML(humlSource);
 * initReactive(template);
 * ```
 */
export function initReactive(template: any): void {
  const reactive = ensureReactiveLoaded();
  reactive.initTemplate(template);
  console.log('[CEL Reactive] Template initialized');
}

/**
 * Update reactive state (triggers automatic reactive propagation)
 *
 * @param name Signal name (from publisherState)
 * @param value New value
 *
 * @example
 * ```typescript
 * updateReactiveState('counter', 5);
 * // This automatically updates dependent computed values!
 * ```
 */
export function updateReactiveState(name: string, value: any): void {
  const reactive = ensureReactiveLoaded();
  reactive.updateState(name, value);
}

/**
 * Get current value of a reactive signal
 *
 * @param name Signal name (from publisherState or publisherComputed)
 * @returns Current value or undefined
 *
 * @example
 * ```typescript
 * const counter = getReactiveValue('counter');
 * const counterDouble = getReactiveValue('counterDouble'); // Computed value
 * ```
 */
export function getReactiveValue(name: string): any {
  const reactive = ensureReactiveLoaded();
  return reactive.getValue(name);
}

/**
 * Get all current reactive values (state + computed)
 *
 * @returns Object with all signal values
 *
 * @example
 * ```typescript
 * const allValues = getAllReactiveValues();
 * console.log(allValues);
 * // { counter: 5, counterDouble: 10, message: "Hello", messageLength: 5 }
 * ```
 */
export function getAllReactiveValues(): Record<string, any> {
  const reactive = ensureReactiveLoaded();
  return reactive.getAllValues();
}

/**
 * Check if CEL reactive is loaded
 */
export function isReactiveLoaded(): boolean {
  return celReactive !== null;
}
