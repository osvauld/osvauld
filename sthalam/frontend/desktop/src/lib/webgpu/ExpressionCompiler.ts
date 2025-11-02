/**
 * HUML Expression → WGSL Compiler
 * Compiles parsed HUML AST to WebGPU compute shaders
 *
 * This is a GENERAL compiler - works with any HUML expression AST
 * No WebGPU-specific syntax needed in HUML!
 */

import type { Expr } from '../types/huml';

interface CompileResult {
  wgslCode: string;
  isGPUCompatible: boolean;
  reason?: string;
}

export class ExpressionToWGSLCompiler {
  /**
   * Check if expression can run on GPU
   * GPU supports: math, arithmetic, simple variables
   * GPU doesn't support: arrays, objects, strings, complex logic
   */
  canCompileToGPU(ast: Expr): boolean {
    return this._checkGPUCompatibility(ast).isCompatible;
  }

  /**
   * Compile HUML AST to WGSL compute shader
   */
  compile(ast: Expr, gridSize: number): CompileResult {
    const compatCheck = this._checkGPUCompatibility(ast);

    if (!compatCheck.isCompatible) {
      return {
        wgslCode: '',
        isGPUCompatible: false,
        reason: compatCheck.reason,
      };
    }

    const expression = this._compileExpression(ast);

    const wgslCode = `
// Auto-generated WGSL compute shader from HUML expression
@group(0) @binding(0) var<storage, read_write> output: array<f32>;
@group(0) @binding(1) var<uniform> params: Params;

struct Params {
  gridSize: u32,
  time: f32,
  mouseX: f32,
  mouseY: f32,
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
  let x = global_id.x;
  let y = global_id.y;
  let gridSize = params.gridSize;
  let time = params.time;
  let mouseX = params.mouseX;
  let mouseY = params.mouseY;

  // Bounds check
  if (x >= gridSize || y >= gridSize) {
    return;
  }

  let index = y * gridSize + x;

  // Compiled HUML expression
  let result = ${expression};

  output[index] = result;
}
`;

    return {
      wgslCode,
      isGPUCompatible: true,
    };
  }

  /**
   * Check if AST is GPU-compatible
   */
  private _checkGPUCompatibility(ast: Expr): { isCompatible: boolean; reason?: string } {
    switch (ast.type) {
      case 'Literal':
        // Only numbers supported
        if (typeof ast.value === 'number') {
          return { isCompatible: true };
        }
        return { isCompatible: false, reason: `Literals must be numbers, got ${typeof ast.value}` };

      case 'Identifier':
        // Only specific variables supported
        const allowed = ['x', 'y', 'time', 'gridSize', 'mouseX', 'mouseY'];
        if (allowed.includes(ast.name)) {
          return { isCompatible: true };
        }
        return { isCompatible: false, reason: `Variable '${ast.name}' not available on GPU` };

      case 'BinaryOp':
        const leftCheck = this._checkGPUCompatibility(ast.left);
        if (!leftCheck.isCompatible) return leftCheck;
        const rightCheck = this._checkGPUCompatibility(ast.right);
        if (!rightCheck.isCompatible) return rightCheck;
        return { isCompatible: true };

      case 'UnaryOp':
        return this._checkGPUCompatibility(ast.operand);

      case 'Call':
        // Only math functions supported
        const gpuFunctions = [
          'sin', 'cos', 'tan', 'sqrt', 'abs', 'pow',
          'atan2', 'min', 'max', 'floor', 'ceil', 'round'
        ];
        if (!gpuFunctions.includes(ast.callee)) {
          return { isCompatible: false, reason: `Function '${ast.callee}' not available on GPU` };
        }
        // Check all arguments
        for (const arg of ast.args) {
          const argCheck = this._checkGPUCompatibility(arg);
          if (!argCheck.isCompatible) return argCheck;
        }
        return { isCompatible: true };

      case 'Conditional':
        // Simple conditionals OK
        const condCheck = this._checkGPUCompatibility(ast.condition);
        if (!condCheck.isCompatible) return condCheck;
        const consCheck = this._checkGPUCompatibility(ast.consequent);
        if (!consCheck.isCompatible) return consCheck;
        const altCheck = this._checkGPUCompatibility(ast.alternate);
        if (!altCheck.isCompatible) return altCheck;
        return { isCompatible: true };

      default:
        return { isCompatible: false, reason: `Expression type '${ast.type}' not supported on GPU` };
    }
  }

  /**
   * Compile AST to WGSL expression
   */
  private _compileExpression(ast: Expr): string {
    switch (ast.type) {
      case 'Literal':
        if (typeof ast.value === 'number') {
          return `${ast.value}`;
        }
        throw new Error(`Unsupported literal type: ${typeof ast.value}`);

      case 'Identifier':
        return `f32(${ast.name})`;

      case 'BinaryOp':
        const left = this._compileExpression(ast.left);
        const right = this._compileExpression(ast.right);
        const op = this._compileOperator(ast.op);
        return `(${left} ${op} ${right})`;

      case 'UnaryOp':
        const operand = this._compileExpression(ast.operand);
        if (ast.op === 'Not') {
          return `!${operand}`;
        } else if (ast.op === 'Neg') {
          return `-${operand}`;
        }
        throw new Error(`Unknown unary operator: ${ast.op}`);

      case 'Call':
        const args = ast.args.map(arg => this._compileExpression(arg)).join(', ');
        return `${ast.callee}(${args})`;

      case 'Conditional':
        const cond = this._compileExpression(ast.condition);
        const cons = this._compileExpression(ast.consequent);
        const alt = this._compileExpression(ast.alternate);
        return `select(${alt}, ${cons}, ${cond})`;

      default:
        throw new Error(`Cannot compile expression type: ${ast.type}`);
    }
  }

  /**
   * Map HUML operators to WGSL operators
   */
  private _compileOperator(op: string): string {
    const opMap: Record<string, string> = {
      'Add': '+',
      'Sub': '-',
      'Mul': '*',
      'Div': '/',
      'Mod': '%',
      'Eq': '==',
      'Ne': '!=',
      'Gt': '>',
      'Lt': '<',
      'Gte': '>=',
      'Lte': '<=',
      'And': '&&',
      'Or': '||',
    };
    return opMap[op] || op;
  }
}

// Export singleton
export const exprCompiler = new ExpressionToWGSLCompiler();
