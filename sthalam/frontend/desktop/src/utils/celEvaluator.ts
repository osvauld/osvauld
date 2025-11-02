/**
 * DEPRECATED: Use humlEvaluator.ts instead
 * This file is kept for backwards compatibility only
 */

// Re-export everything from the new HUML evaluator
export {
  evaluateExpression,
  evaluateValue,
  hasExpression as hasCELExpression,
  evaluateCEL,
} from './humlEvaluator';
