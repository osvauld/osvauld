/**
 * HUML Expression AST Types
 * Matches OCaml evaluator expression types
 */

export type Expr =
  | { type: 'Literal'; value: number | string | boolean | null }
  | { type: 'Identifier'; name: string }
  | { type: 'BinaryOp'; op: string; left: Expr; right: Expr }
  | { type: 'UnaryOp'; op: string; operand: Expr }
  | { type: 'Call'; callee: string; args: Expr[] }
  | { type: 'Conditional'; condition: Expr; consequent: Expr; alternate: Expr }
  | { type: 'ArrayLiteral'; elements: Expr[] }
  | { type: 'ObjectLiteral'; properties: Record<string, Expr> }
  | { type: 'MemberAccess'; object: Expr; property: string }
  | { type: 'ArrayAccess'; array: Expr; index: Expr };

/**
 * HUML Value Types
 * Matches OCaml evaluator value types
 */
export type Value =
  | { type: 'VNull' }
  | { type: 'VInt'; value: number }
  | { type: 'VFloat'; value: number }
  | { type: 'VBool'; value: boolean }
  | { type: 'VString'; value: string }
  | { type: 'VArray'; elements: Value[] }
  | { type: 'VObject'; properties: Record<string, Value> };
