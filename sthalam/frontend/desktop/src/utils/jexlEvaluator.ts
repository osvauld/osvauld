import jexl from 'jexl';
import { templateState } from '../lib/templateState.svelte';

/**
 * JEXL Expression Evaluator for HUML Templates
 *
 * Evaluates JEXL expressions in block properties using template state.
 * Supports expressions in {{...}} syntax.
 */

// Configure JEXL instance (use singleton directly)
const jexlInstance = jexl;

// Add custom transforms
jexlInstance.addTransform('uppercase', (val: string) => val?.toUpperCase());
jexlInstance.addTransform('lowercase', (val: string) => val?.toLowerCase());
jexlInstance.addTransform('capitalize', (val: string) =>
  val ? val.charAt(0).toUpperCase() + val.slice(1).toLowerCase() : ''
);
jexlInstance.addTransform('toNumber', (val: any) => {
  const num = Number(val);
  return isNaN(num) ? 0 : num;
});
jexlInstance.addTransform('toFixed', (val: any, decimals: number = 2) => {
  const num = Number(val);
  return isNaN(num) ? '0.00' : num.toFixed(decimals);
});

// Array transforms
jexlInstance.addTransform('length', (val: any) => {
  if (Array.isArray(val) || typeof val === 'string') {
    return val.length;
  }
  if (val && typeof val === 'object') {
    return Object.keys(val).length;
  }
  return 0;
});
jexlInstance.addTransform('first', (arr: any[]) => Array.isArray(arr) ? arr[0] : undefined);
jexlInstance.addTransform('last', (arr: any[]) => Array.isArray(arr) ? arr[arr.length - 1] : undefined);
jexlInstance.addTransform('join', (arr: any[], separator: string = ',') =>
  Array.isArray(arr) ? arr.join(separator) : ''
);
jexlInstance.addTransform('reverse', (arr: any[]) =>
  Array.isArray(arr) ? [...arr].reverse() : []
);
jexlInstance.addTransform('sort', (arr: any[]) =>
  Array.isArray(arr) ? [...arr].sort() : []
);
jexlInstance.addTransform('unique', (arr: any[]) =>
  Array.isArray(arr) ? [...new Set(arr)] : []
);

// Add safe JavaScript functions to JEXL context
// Only whitelisted functions are exposed - prevents access to dangerous APIs
jexlInstance.addFunction('Number', (val: any) => Number(val));
jexlInstance.addFunction('String', (val: any) => String(val));
jexlInstance.addFunction('Boolean', (val: any) => Boolean(val));
jexlInstance.addFunction('parseInt', (val: string, radix?: number) => parseInt(val, radix));
jexlInstance.addFunction('parseFloat', (val: string) => parseFloat(val));
jexlInstance.addFunction('isNaN', (val: any) => isNaN(val));

// Add safe Math functions (no dangerous operations)
jexlInstance.addFunction('Math.round', (val: number) => Math.round(val));
jexlInstance.addFunction('Math.floor', (val: number) => Math.floor(val));
jexlInstance.addFunction('Math.ceil', (val: number) => Math.ceil(val));
jexlInstance.addFunction('Math.abs', (val: number) => Math.abs(val));
jexlInstance.addFunction('Math.min', (...args: number[]) => Math.min(...args));
jexlInstance.addFunction('Math.max', (...args: number[]) => Math.max(...args));
jexlInstance.addFunction('Math.pow', (base: number, exp: number) => Math.pow(base, exp));
jexlInstance.addFunction('Math.sqrt', (val: number) => Math.sqrt(val));
jexlInstance.addFunction('Math.random', () => Math.random());

// Array helper functions
jexlInstance.addFunction('length', (val: any) => {
  if (Array.isArray(val) || typeof val === 'string') {
    return val.length;
  }
  if (val && typeof val === 'object') {
    return Object.keys(val).length;
  }
  return 0;
});
jexlInstance.addFunction('sum', (arr: any[], prop?: string) => {
  if (!Array.isArray(arr)) return 0;
  if (prop) {
    return arr.reduce((sum, item) => sum + (Number(item[prop]) || 0), 0);
  }
  return arr.reduce((sum, item) => sum + (Number(item) || 0), 0);
});
jexlInstance.addFunction('avg', (arr: any[], prop?: string) => {
  if (!Array.isArray(arr) || arr.length === 0) return 0;
  const total = jexlInstance.evalSync('sum(arr, prop)', { arr, prop, sum: jexlInstance._functions.sum });
  return total / arr.length;
});
jexlInstance.addFunction('count', (arr: any[], condition?: string) => {
  if (!Array.isArray(arr)) return 0;
  if (!condition) return arr.length;
  // Simple condition check: count items where a property is truthy
  return arr.filter(item => item[condition]).length;
});
jexlInstance.addFunction('contains', (arr: any[], value: any) => {
  if (!Array.isArray(arr)) return false;
  return arr.includes(value);
});

// Object helper functions
jexlInstance.addFunction('keys', (obj: any) => {
  if (obj && typeof obj === 'object') {
    return Object.keys(obj);
  }
  return [];
});
jexlInstance.addFunction('values', (obj: any) => {
  if (obj && typeof obj === 'object') {
    return Object.values(obj);
  }
  return [];
});

/**
 * Check if a string contains JEXL expressions
 */
export function hasJEXL(value: string): boolean {
  if (typeof value !== 'string') return false;
  return /\{\{.+?\}\}/.test(value);
}

/**
 * Extract JEXL expression from {{...}} syntax
 */
export function extractExpression(value: string): string | null {
  const match = value.match(/^\{\{(.+?)\}\}$/);
  return match ? match[1].trim() : null;
}

/**
 * Evaluate a JEXL expression with current template state (SYNCHRONOUS)
 * @param expression - The JEXL expression to evaluate
 * @param additionalContext - Optional additional context (e.g., loop variables like item, index)
 */
export function evaluateExpression(expression: string, additionalContext?: Record<string, any>): any {
  try {
    const state = templateState.get();
    // Merge template state with additional context (additional context takes precedence)
    const context = additionalContext ? { ...state, ...additionalContext } : state;
    const result = jexlInstance.evalSync(expression, context);
    return result;
  } catch (error) {
    console.error(`❌ [JEXL] Failed to evaluate expression: ${expression}`, error);
    return undefined;
  }
}

/**
 * Evaluate a value that may contain JEXL expressions (SYNCHRONOUS)
 * Handles three cases:
 * 1. "{{expression}}" - Full expression, returns evaluated result
 * 2. "text {{expr}} more text" - Interpolation, returns string with evaluated parts
 * 3. "plain text" - No expression, returns as-is
 * @param value - The value to evaluate
 * @param additionalContext - Optional additional context (e.g., loop variables)
 */
export function evaluateValue(value: any, additionalContext?: Record<string, any>): any {
  // Non-string values pass through
  if (typeof value !== 'string') {
    return value;
  }

  // No JEXL expression - return as-is
  if (!hasJEXL(value)) {
    return value;
  }

  // Case 1: Full expression "{{expression}}"
  const fullExpression = extractExpression(value);
  if (fullExpression) {
    return evaluateExpression(fullExpression, additionalContext);
  }

  // Case 2: Interpolation "text {{expr}} more text"
  const parts: (string | any)[] = [];
  let lastIndex = 0;
  const regex = /\{\{(.+?)\}\}/g;
  let match;

  while ((match = regex.exec(value)) !== null) {
    // Add text before the expression
    if (match.index > lastIndex) {
      parts.push(value.substring(lastIndex, match.index));
    }

    // Evaluate the expression
    const expression = match[1].trim();
    const result = evaluateExpression(expression, additionalContext);
    parts.push(result);

    lastIndex = match.index + match[0].length;
  }

  // Add remaining text after last expression
  if (lastIndex < value.length) {
    parts.push(value.substring(lastIndex));
  }

  // Join all parts into a string
  return parts.join('');
}

/**
 * Evaluate all JEXL expressions in a block's properties (SYNCHRONOUS)
 * Returns a new object with evaluated values
 */
export function evaluateBlockProperties(
  blockData: Record<string, any>
): Record<string, any> {
  const evaluated: Record<string, any> = {};

  for (const [key, value] of Object.entries(blockData)) {
    // Recursively evaluate nested objects
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      evaluated[key] = evaluateBlockProperties(value);
    }
    // Evaluate arrays
    else if (Array.isArray(value)) {
      evaluated[key] = value.map(item =>
        typeof item === 'object' ? evaluateBlockProperties(item) : evaluateValue(item)
      );
    }
    // Evaluate string values
    else {
      evaluated[key] = evaluateValue(value);
    }
  }

  return evaluated;
}

/**
 * Evaluate a specific property with JEXL (SYNCHRONOUS)
 * Convenience function for common use cases
 */
export function evaluateProperty(
  blockData: Record<string, any>,
  propertyName: string
): any {
  const value = blockData[propertyName];
  if (value === undefined) return undefined;
  return evaluateValue(value);
}

/**
 * Evaluate visibility condition
 * Common use case for showing/hiding blocks
 */
export function evaluateVisibility(
  blockData: Record<string, any>
): boolean {
  const visible = blockData.visible;

  // If visible is undefined or true, show by default
  if (visible === undefined || visible === true) {
    return true;
  }

  // If visible is false, hide
  if (visible === false) {
    return false;
  }

  // If visible is a JEXL expression, evaluate it
  if (typeof visible === 'string' && hasJEXL(visible)) {
    const result = evaluateValue(visible);
    return Boolean(result);
  }

  // Otherwise, coerce to boolean
  return Boolean(visible);
}

// Export jexl instance for advanced usage
export { jexlInstance };
