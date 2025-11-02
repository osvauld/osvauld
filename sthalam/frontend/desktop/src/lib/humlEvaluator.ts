/**
 * HUML Expression Evaluator
 * OCaml-powered expression evaluation with type safety
 */

// Type definitions
interface EvalResult {
  output: string;
  error: string;
  success: boolean;
}

interface HumlEvalGlobal {
  evaluate: (expression: string, context: string) => EvalResult;
}

declare global {
  const humlEval: HumlEvalGlobal;
}

// Load the OCaml evaluator script dynamically from public folder
let evaluatorLoaded = false;
let evaluatorLoading = false;

function loadEvaluator(): Promise<void> {
  if (evaluatorLoaded) return Promise.resolve();
  if (evaluatorLoading) {
    // Wait for it to load
    return new Promise((resolve) => {
      const interval = setInterval(() => {
        if (evaluatorLoaded) {
          clearInterval(interval);
          resolve();
        }
      }, 50);
    });
  }

  evaluatorLoading = true;

  return new Promise((resolve, reject) => {
    const script = document.createElement('script');
    // Use asset:// protocol in Tauri for proper MIME type handling
    const isTauri = typeof window !== 'undefined' && '__TAURI__' in window;
    script.src = isTauri ? 'asset://localhost/huml_eval.js' : '/huml_eval.js';
    console.log(`[HUML Eval] Loading from: ${script.src} (isTauri: ${isTauri})`);
    script.onload = () => {
      evaluatorLoaded = true;
      evaluatorLoading = false;
      console.log('✅ [HUML Eval] Evaluator loaded successfully');
      resolve();
    };
    script.onerror = (error) => {
      evaluatorLoading = false;
      console.error('❌ [HUML Eval] Failed to load evaluator:', error);
      reject(new Error('Failed to load huml_eval.js'));
    };
    document.head.appendChild(script);
  });
}

// Start loading immediately
loadEvaluator().catch(console.error);

/**
 * Evaluate HUML expression with context
 * @param expression - Expression string (without {{ }})
 * @param context - Context object with variables
 * @returns Evaluated result
 */
export function evaluateExpression(
  expression: string,
  context: Record<string, any> = {}
): any {
  try {
    // Safety check: ensure humlEval is loaded
    if (typeof humlEval === 'undefined') {
      return null;
    }

    const result = humlEval.evaluate(expression, JSON.stringify(context));

    if (result.success) {
      return JSON.parse(result.output);
    } else {
      console.error('[HUML Eval] Error evaluating:', expression, '→', result.error);
      return null;
    }
  } catch (error) {
    console.error('[HUML Eval] Exception:', expression, '→', error);
    return null;
  }
}

/**
 * Evaluate value that might contain expression
 * Handles {{ }} wrapper automatically
 */
export function evaluateValue(
  value: any,
  context: Record<string, any> = {}
): any {
  if (typeof value !== 'string') return value;

  const trimmed = value.trim();
  if (trimmed.startsWith('{{') && trimmed.endsWith('}}')) {
    const expr = trimmed.slice(2, -2).trim();
    return evaluateExpression(expr, context);
  }

  return value;
}

/**
 * Check if value contains expression
 */
export function hasExpression(value: any): boolean {
  if (typeof value !== 'string') return false;
  const trimmed = value.trim();
  return trimmed.startsWith('{{') && trimmed.endsWith('}}');
}
