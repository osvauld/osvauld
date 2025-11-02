/**
 * HUML Expression Language Evaluator v0.1.0
 *
 * A custom expression evaluator designed to match HUML's philosophy:
 * - Human-first readability
 * - Visual clarity
 * - Eliminated ambiguity
 * - Singular representation
 * - Inherent consistency
 *
 * See HUML_EXPRESSION_SPEC.md for full specification
 */

// ============================================================================
// TOKEN TYPES
// ============================================================================

enum TokenType {
  // Literals
  NUMBER = 'NUMBER',
  STRING = 'STRING',
  TRUE = 'TRUE',
  FALSE = 'FALSE',
  NULL = 'NULL',
  IDENTIFIER = 'IDENTIFIER',

  // Operators
  PLUS = 'PLUS',             // +
  MINUS = 'MINUS',           // -
  MULTIPLY = 'MULTIPLY',     // *
  DIVIDE = 'DIVIDE',         // /
  MODULO = 'MODULO',         // %

  // Comparison
  EQUALS = 'EQUALS',         // ==
  NOT_EQUALS = 'NOT_EQUALS', // !=
  GT = 'GT',                 // >
  LT = 'LT',                 // <
  GTE = 'GTE',               // >=
  LTE = 'LTE',               // <=

  // Logical (keywords only)
  AND = 'AND',               // and
  OR = 'OR',                 // or
  NOT = 'NOT',               // not

  // State checks
  IS = 'IS',                 // is
  IN = 'IN',                 // in
  EMPTY = 'EMPTY',           // empty
  OF = 'OF',                 // of

  // Keyword alternatives
  GREATER = 'GREATER',       // greater
  LESS = 'LESS',             // less
  THAN = 'THAN',             // than
  AT = 'AT',                 // at
  LEAST = 'LEAST',           // least
  MOST = 'MOST',             // most

  // Symbols
  DOT = 'DOT',               // .
  COMMA = 'COMMA',           // ,
  QUESTION = 'QUESTION',     // ?
  COLON = 'COLON',           // :
  LPAREN = 'LPAREN',         // (
  RPAREN = 'RPAREN',         // )
  LBRACKET = 'LBRACKET',     // [
  RBRACKET = 'RBRACKET',     // ]

  EOF = 'EOF',
}

interface Token {
  type: TokenType;
  value: any;
  pos: number;
}

// ============================================================================
// TOKENIZER
// ============================================================================

class Tokenizer {
  private input: string;
  private pos: number = 0;
  private current: string | null = null;

  constructor(input: string) {
    this.input = input;
    this.current = input[0] || null;
  }

  private advance(): void {
    this.pos++;
    this.current = this.pos < this.input.length ? this.input[this.pos] : null;
  }

  private peek(offset: number = 1): string | null {
    const peekPos = this.pos + offset;
    return peekPos < this.input.length ? this.input[peekPos] : null;
  }

  private skipWhitespace(): void {
    while (this.current !== null && /\s/.test(this.current)) {
      this.advance();
    }
  }

  private readNumber(): Token {
    const start = this.pos;
    let numStr = '';

    // Handle negative numbers
    if (this.current === '-') {
      numStr += '-';
      this.advance();
    }

    // Read digits and decimal point
    while (this.current !== null && /[0-9.]/.test(this.current)) {
      numStr += this.current;
      this.advance();
    }

    return {
      type: TokenType.NUMBER,
      value: parseFloat(numStr),
      pos: start,
    };
  }

  private readString(quote: string): Token {
    const start = this.pos;
    let str = '';
    this.advance(); // Skip opening quote

    while (this.current !== null && this.current !== quote) {
      if (this.current === '\\' && this.peek() === quote) {
        // Handle escaped quotes
        this.advance();
        str += quote;
        this.advance();
      } else if (this.current === '\\' && this.peek() === '\\') {
        // Handle escaped backslash
        this.advance();
        str += '\\';
        this.advance();
      } else {
        str += this.current;
        this.advance();
      }
    }

    if (this.current === quote) {
      this.advance(); // Skip closing quote
    }

    return {
      type: TokenType.STRING,
      value: str,
      pos: start,
    };
  }

  private readIdentifier(): Token {
    const start = this.pos;
    let id = '';

    while (this.current !== null && /[a-zA-Z0-9_]/.test(this.current)) {
      id += this.current;
      this.advance();
    }

    // Check for keywords
    const keywords: Record<string, TokenType> = {
      'true': TokenType.TRUE,
      'false': TokenType.FALSE,
      'null': TokenType.NULL,
      'and': TokenType.AND,
      'or': TokenType.OR,
      'not': TokenType.NOT,
      'is': TokenType.IS,
      'in': TokenType.IN,
      'empty': TokenType.EMPTY,
      'of': TokenType.OF,
      'equals': TokenType.EQUALS,
      'greater': TokenType.GREATER,
      'less': TokenType.LESS,
      'than': TokenType.THAN,
      'at': TokenType.AT,
      'least': TokenType.LEAST,
      'most': TokenType.MOST,
    };

    const type = keywords[id] || TokenType.IDENTIFIER;

    return {
      type,
      value: id,
      pos: start,
    };
  }

  tokenize(): Token[] {
    const tokens: Token[] = [];

    while (this.current !== null) {
      this.skipWhitespace();

      if (this.current === null) break;

      // Numbers
      if (/[0-9]/.test(this.current) || (this.current === '-' && this.peek() && /[0-9]/.test(this.peek()!))) {
        tokens.push(this.readNumber());
        continue;
      }

      // Strings
      if (this.current === '"' || this.current === "'") {
        tokens.push(this.readString(this.current));
        continue;
      }

      // Identifiers and keywords
      if (/[a-zA-Z_]/.test(this.current)) {
        tokens.push(this.readIdentifier());
        continue;
      }

      // Two-character operators
      if (this.current === '=' && this.peek() === '=') {
        tokens.push({ type: TokenType.EQUALS, value: '==', pos: this.pos });
        this.advance();
        this.advance();
        continue;
      }

      if (this.current === '!' && this.peek() === '=') {
        tokens.push({ type: TokenType.NOT_EQUALS, value: '!=', pos: this.pos });
        this.advance();
        this.advance();
        continue;
      }

      if (this.current === '>' && this.peek() === '=') {
        tokens.push({ type: TokenType.GTE, value: '>=', pos: this.pos });
        this.advance();
        this.advance();
        continue;
      }

      if (this.current === '<' && this.peek() === '=') {
        tokens.push({ type: TokenType.LTE, value: '<=', pos: this.pos });
        this.advance();
        this.advance();
        continue;
      }

      // Single-character operators and symbols
      const singleChar: Record<string, TokenType> = {
        '+': TokenType.PLUS,
        '-': TokenType.MINUS,
        '*': TokenType.MULTIPLY,
        '/': TokenType.DIVIDE,
        '%': TokenType.MODULO,
        '>': TokenType.GT,
        '<': TokenType.LT,
        '.': TokenType.DOT,
        ',': TokenType.COMMA,
        '?': TokenType.QUESTION,
        ':': TokenType.COLON,
        '(': TokenType.LPAREN,
        ')': TokenType.RPAREN,
        '[': TokenType.LBRACKET,
        ']': TokenType.RBRACKET,
      };

      if (singleChar[this.current]) {
        tokens.push({ type: singleChar[this.current], value: this.current, pos: this.pos });
        this.advance();
        continue;
      }

      // Unknown character
      throw new Error(`Unexpected character '${this.current}' at position ${this.pos}`);
    }

    tokens.push({ type: TokenType.EOF, value: null, pos: this.pos });
    return tokens;
  }
}

// ============================================================================
// AST NODE TYPES
// ============================================================================

interface ASTNode {
  type: string;
}

interface LiteralNode extends ASTNode {
  type: 'Literal';
  value: any;
}

interface IdentifierNode extends ASTNode {
  type: 'Identifier';
  name: string;
}

interface BinaryOpNode extends ASTNode {
  type: 'BinaryOp';
  operator: string;
  left: ASTNode;
  right: ASTNode;
}

interface UnaryOpNode extends ASTNode {
  type: 'UnaryOp';
  operator: string;
  operand: ASTNode;
}

interface MemberAccessNode extends ASTNode {
  type: 'MemberAccess';
  object: ASTNode;
  property: ASTNode;
  computed: boolean; // true for bracket notation
}

interface CallNode extends ASTNode {
  type: 'Call';
  callee: string;
  args: ASTNode[];
}

interface ConditionalNode extends ASTNode {
  type: 'Conditional';
  condition: ASTNode;
  consequent: ASTNode;
  alternate: ASTNode;
}

interface StateCheckNode extends ASTNode {
  type: 'StateCheck';
  operator: 'isEmpty' | 'isNotEmpty' | 'in';
  left: ASTNode;
  right?: ASTNode;
}

// ============================================================================
// PARSER
// ============================================================================

class Parser {
  private tokens: Token[];
  private pos: number = 0;
  private current: Token;

  constructor(tokens: Token[]) {
    this.tokens = tokens;
    this.current = tokens[0];
  }

  private advance(): void {
    this.pos++;
    this.current = this.tokens[this.pos];
  }

  private expect(type: TokenType): Token {
    if (this.current.type !== type) {
      throw new Error(`Expected ${type} but got ${this.current.type} at position ${this.current.pos}`);
    }
    const token = this.current;
    this.advance();
    return token;
  }

  private match(...types: TokenType[]): boolean {
    return types.includes(this.current.type);
  }

  private isKeywordAsIdentifier(): boolean {
    // Some keywords can be used as identifiers in certain contexts
    // (e.g., "empty" as a variable name in "size of empty")
    const allowedKeywords = [
      TokenType.EMPTY,
      TokenType.OF,
      TokenType.EQUALS,
      TokenType.GREATER,
      TokenType.LESS,
      TokenType.THAN,
      TokenType.AT,
      TokenType.LEAST,
      TokenType.MOST,
    ];
    return allowedKeywords.includes(this.current.type);
  }

  parse(): ASTNode {
    return this.parseExpression();
  }

  // Expression (lowest precedence)
  private parseExpression(): ASTNode {
    return this.parseTernary();
  }

  // Ternary: condition ? consequent : alternate
  private parseTernary(): ASTNode {
    let node = this.parseLogicalOr();

    if (this.match(TokenType.QUESTION)) {
      this.advance();
      const consequent = this.parseExpression();
      this.expect(TokenType.COLON);
      const alternate = this.parseExpression();

      return {
        type: 'Conditional',
        condition: node,
        consequent,
        alternate,
      } as ConditionalNode;
    }

    return node;
  }

  // Logical OR
  private parseLogicalOr(): ASTNode {
    let left = this.parseLogicalAnd();

    while (this.match(TokenType.OR)) {
      this.advance();
      const right = this.parseLogicalAnd();
      left = {
        type: 'BinaryOp',
        operator: 'or',
        left,
        right,
      } as BinaryOpNode;
    }

    return left;
  }

  // Logical AND
  private parseLogicalAnd(): ASTNode {
    let left = this.parseEquality();

    while (this.match(TokenType.AND)) {
      this.advance();
      const right = this.parseEquality();
      left = {
        type: 'BinaryOp',
        operator: 'and',
        left,
        right,
      } as BinaryOpNode;
    }

    return left;
  }

  // Equality: ==, !=, equals, not equals
  private parseEquality(): ASTNode {
    let left = this.parseComparison();

    while (this.match(TokenType.EQUALS, TokenType.NOT_EQUALS)) {
      const operator = this.current.type === TokenType.EQUALS ? '==' : '!=';
      this.advance();

      // Handle "not equals" keyword
      if (operator === '!=' && this.match(TokenType.EQUALS)) {
        this.advance();
      }

      const right = this.parseComparison();
      left = {
        type: 'BinaryOp',
        operator,
        left,
        right,
      } as BinaryOpNode;
    }

    return left;
  }

  // Comparison: >, <, >=, <=, greater than, less than, at least, at most
  private parseComparison(): ASTNode {
    let left = this.parseStateCheck();

    // Handle keyword comparisons
    if (this.match(TokenType.GREATER, TokenType.LESS, TokenType.AT)) {
      let operator = '';

      if (this.match(TokenType.GREATER)) {
        this.advance();
        this.expect(TokenType.THAN);
        operator = '>';
      } else if (this.match(TokenType.LESS)) {
        this.advance();
        this.expect(TokenType.THAN);
        operator = '<';
      } else if (this.match(TokenType.AT)) {
        this.advance();
        if (this.match(TokenType.LEAST)) {
          this.advance();
          operator = '>=';
        } else if (this.match(TokenType.MOST)) {
          this.advance();
          operator = '<=';
        }
      }

      const right = this.parseStateCheck();
      return {
        type: 'BinaryOp',
        operator,
        left,
        right,
      } as BinaryOpNode;
    }

    // Handle symbol comparisons
    while (this.match(TokenType.GT, TokenType.LT, TokenType.GTE, TokenType.LTE)) {
      const opMap: Record<string, string> = {
        [TokenType.GT]: '>',
        [TokenType.LT]: '<',
        [TokenType.GTE]: '>=',
        [TokenType.LTE]: '<=',
      };
      const operator = opMap[this.current.type];
      this.advance();
      const right = this.parseStateCheck();
      left = {
        type: 'BinaryOp',
        operator,
        left,
        right,
      } as BinaryOpNode;
    }

    return left;
  }

  // State checks: is empty, is not empty, in
  private parseStateCheck(): ASTNode {
    let left = this.parseAdditive();

    // Handle "is empty" / "is not empty"
    if (this.match(TokenType.IS)) {
      this.advance();

      if (this.match(TokenType.NOT)) {
        this.advance();
        this.expect(TokenType.EMPTY);
        return {
          type: 'StateCheck',
          operator: 'isNotEmpty',
          left,
        } as StateCheckNode;
      } else if (this.match(TokenType.EMPTY)) {
        this.advance();
        return {
          type: 'StateCheck',
          operator: 'isEmpty',
          left,
        } as StateCheckNode;
      }

      // Fall back to treating "is" as an equality operator (though not standard)
      throw new Error(`Expected 'empty' or 'not empty' after 'is' at position ${this.current.pos}`);
    }

    // Handle "in array"
    if (this.match(TokenType.IN)) {
      this.advance();
      const right = this.parseAdditive();
      return {
        type: 'StateCheck',
        operator: 'in',
        left,
        right,
      } as StateCheckNode;
    }

    return left;
  }

  // Additive: +, -
  private parseAdditive(): ASTNode {
    let left = this.parseMultiplicative();

    while (this.match(TokenType.PLUS, TokenType.MINUS)) {
      const operator = this.current.value;
      this.advance();
      const right = this.parseMultiplicative();
      left = {
        type: 'BinaryOp',
        operator,
        left,
        right,
      } as BinaryOpNode;
    }

    return left;
  }

  // Multiplicative: *, /, %
  private parseMultiplicative(): ASTNode {
    let left = this.parseUnary();

    while (this.match(TokenType.MULTIPLY, TokenType.DIVIDE, TokenType.MODULO)) {
      const operator = this.current.value;
      this.advance();
      const right = this.parseUnary();
      left = {
        type: 'BinaryOp',
        operator,
        left,
        right,
      } as BinaryOpNode;
    }

    return left;
  }

  // Unary: not, -
  private parseUnary(): ASTNode {
    if (this.match(TokenType.NOT)) {
      this.advance();
      const operand = this.parseUnary();
      return {
        type: 'UnaryOp',
        operator: 'not',
        operand,
      } as UnaryOpNode;
    }

    if (this.match(TokenType.MINUS)) {
      this.advance();
      const operand = this.parseUnary();
      return {
        type: 'UnaryOp',
        operator: '-',
        operand,
      } as UnaryOpNode;
    }

    return this.parseFunctionOrMemberAccess();
  }

  // Function calls or member access
  private parseFunctionOrMemberAccess(): ASTNode {
    let node = this.parsePrimary();

    // Handle function calls with "of" syntax: size of array
    // This is handled in parsePrimary for now

    // Handle member access and bracket notation
    while (this.match(TokenType.DOT, TokenType.LBRACKET)) {
      if (this.match(TokenType.DOT)) {
        this.advance();
        const property = this.expect(TokenType.IDENTIFIER);
        node = {
          type: 'MemberAccess',
          object: node,
          property: { type: 'Identifier', name: property.value } as IdentifierNode,
          computed: false,
        } as MemberAccessNode;
      } else if (this.match(TokenType.LBRACKET)) {
        this.advance();
        const property = this.parseExpression();
        this.expect(TokenType.RBRACKET);
        node = {
          type: 'MemberAccess',
          object: node,
          property,
          computed: true,
        } as MemberAccessNode;
      }
    }

    return node;
  }

  // Primary: literals, identifiers, function calls, parenthesized expressions
  private parsePrimary(): ASTNode {
    // Literals
    if (this.match(TokenType.NUMBER)) {
      const value = this.current.value;
      this.advance();
      return { type: 'Literal', value } as LiteralNode;
    }

    if (this.match(TokenType.STRING)) {
      const value = this.current.value;
      this.advance();
      return { type: 'Literal', value } as LiteralNode;
    }

    if (this.match(TokenType.TRUE)) {
      this.advance();
      return { type: 'Literal', value: true } as LiteralNode;
    }

    if (this.match(TokenType.FALSE)) {
      this.advance();
      return { type: 'Literal', value: false } as LiteralNode;
    }

    if (this.match(TokenType.NULL)) {
      this.advance();
      return { type: 'Literal', value: null } as LiteralNode;
    }

    // Function calls (traditional and "of" syntax)
    // Also handle keywords that can be used as identifiers (like "empty" as a variable name)
    if (this.match(TokenType.IDENTIFIER) || this.isKeywordAsIdentifier()) {
      const name = this.current.value;
      this.advance();

      // Check for "of" syntax: size of array, length of string
      if (this.match(TokenType.OF)) {
        this.advance();
        const arg = this.parseAdditive(); // Parse the argument
        return {
          type: 'Call',
          callee: name,
          args: [arg],
        } as CallNode;
      }

      // Traditional function call: func(arg1, arg2)
      if (this.match(TokenType.LPAREN)) {
        this.advance();
        const args: ASTNode[] = [];

        if (!this.match(TokenType.RPAREN)) {
          args.push(this.parseExpression());
          while (this.match(TokenType.COMMA)) {
            this.advance();
            args.push(this.parseExpression());
          }
        }

        this.expect(TokenType.RPAREN);
        return {
          type: 'Call',
          callee: name,
          args,
        } as CallNode;
      }

      // Just an identifier
      return { type: 'Identifier', name } as IdentifierNode;
    }

    // Parenthesized expression
    if (this.match(TokenType.LPAREN)) {
      this.advance();
      const node = this.parseExpression();
      this.expect(TokenType.RPAREN);
      return node;
    }

    throw new Error(`Unexpected token ${this.current.type} at position ${this.current.pos}`);
  }
}

// ============================================================================
// EVALUATOR
// ============================================================================

class Evaluator {
  private context: Record<string, any>;

  constructor(context: Record<string, any> = {}) {
    this.context = this.addBuiltins(context);
  }

  private addBuiltins(context: Record<string, any>): Record<string, any> {
    return {
      ...context,
      // Built-in functions are handled in evaluateCall
    };
  }

  evaluate(node: ASTNode): any {
    switch (node.type) {
      case 'Literal':
        return (node as LiteralNode).value;

      case 'Identifier':
        return this.evaluateIdentifier(node as IdentifierNode);

      case 'BinaryOp':
        return this.evaluateBinaryOp(node as BinaryOpNode);

      case 'UnaryOp':
        return this.evaluateUnaryOp(node as UnaryOpNode);

      case 'MemberAccess':
        return this.evaluateMemberAccess(node as MemberAccessNode);

      case 'Call':
        return this.evaluateCall(node as CallNode);

      case 'Conditional':
        return this.evaluateConditional(node as ConditionalNode);

      case 'StateCheck':
        return this.evaluateStateCheck(node as StateCheckNode);

      default:
        throw new Error(`Unknown node type: ${node.type}`);
    }
  }

  private evaluateIdentifier(node: IdentifierNode): any {
    const value = this.context[node.name];
    return value !== undefined ? value : null;
  }

  private evaluateBinaryOp(node: BinaryOpNode): any {
    const left = this.evaluate(node.left);
    const right = this.evaluate(node.right);

    switch (node.operator) {
      // Arithmetic
      case '+':
        return left + right;
      case '-':
        return left - right;
      case '*':
        return left * right;
      case '/':
        return left / right;
      case '%':
        return left % right;

      // Comparison
      case '>':
        return left > right;
      case '<':
        return left < right;
      case '>=':
        return left >= right;
      case '<=':
        return left <= right;
      case '==':
        return left == right;
      case '!=':
        return left != right;

      // Logical
      case 'and':
        return left && right;
      case 'or':
        return left || right;

      default:
        throw new Error(`Unknown binary operator: ${node.operator}`);
    }
  }

  private evaluateUnaryOp(node: UnaryOpNode): any {
    const operand = this.evaluate(node.operand);

    switch (node.operator) {
      case 'not':
        return !operand;
      case '-':
        return -operand;
      default:
        throw new Error(`Unknown unary operator: ${node.operator}`);
    }
  }

  private evaluateMemberAccess(node: MemberAccessNode): any {
    const object = this.evaluate(node.object);

    if (object === null || object === undefined) {
      return null;
    }

    if (node.computed) {
      // Bracket notation: obj[expr]
      const property = this.evaluate(node.property);
      return object[property] !== undefined ? object[property] : null;
    } else {
      // Dot notation: obj.prop
      const propertyName = (node.property as IdentifierNode).name;
      return object[propertyName] !== undefined ? object[propertyName] : null;
    }
  }

  private evaluateCall(node: CallNode): any {
    const args = node.args.map(arg => this.evaluate(arg));

    // Built-in functions
    switch (node.callee) {
      // Collection functions
      case 'size':
        return Array.isArray(args[0]) ? args[0].length : 0;

      case 'length':
        return typeof args[0] === 'string' ? args[0].length : 0;

      case 'contains':
        return Array.isArray(args[0]) ? args[0].includes(args[1]) : false;

      case 'first':
        return Array.isArray(args[0]) && args[0].length > 0 ? args[0][0] : null;

      case 'last':
        return Array.isArray(args[0]) && args[0].length > 0 ? args[0][args[0].length - 1] : null;

      // Type conversion
      case 'string':
        return String(args[0]);

      case 'number':
        return Number(args[0]);

      case 'boolean':
        return Boolean(args[0]);

      // String functions
      case 'uppercase':
        return typeof args[0] === 'string' ? args[0].toUpperCase() : '';

      case 'lowercase':
        return typeof args[0] === 'string' ? args[0].toLowerCase() : '';

      case 'trim':
        return typeof args[0] === 'string' ? args[0].trim() : '';

      // Math functions
      case 'abs':
        return Math.abs(args[0]);

      case 'round':
        return Math.round(args[0]);

      case 'floor':
        return Math.floor(args[0]);

      case 'ceil':
        return Math.ceil(args[0]);

      case 'min':
        return Math.min(args[0], args[1]);

      case 'max':
        return Math.max(args[0], args[1]);

      default:
        console.warn(`Unknown function: ${node.callee}`);
        return null;
    }
  }

  private evaluateConditional(node: ConditionalNode): any {
    const condition = this.evaluate(node.condition);
    return condition ? this.evaluate(node.consequent) : this.evaluate(node.alternate);
  }

  private evaluateStateCheck(node: StateCheckNode): any {
    const left = this.evaluate(node.left);

    switch (node.operator) {
      case 'isEmpty':
        if (typeof left === 'string') {
          return left === '';
        }
        if (Array.isArray(left)) {
          return left.length === 0;
        }
        return left === null || left === undefined;

      case 'isNotEmpty':
        if (typeof left === 'string') {
          return left !== '';
        }
        if (Array.isArray(left)) {
          return left.length > 0;
        }
        return left !== null && left !== undefined;

      case 'in':
        const right = this.evaluate(node.right!);
        return Array.isArray(right) ? right.includes(left) : false;

      default:
        throw new Error(`Unknown state check operator: ${node.operator}`);
    }
  }
}

// ============================================================================
// PUBLIC API
// ============================================================================

/**
 * Evaluate a HUML expression with the given context
 * @param expression - The expression to evaluate (without {{ }})
 * @param context - The context object containing variables
 * @returns The evaluated result
 */
export function evaluateExpression(expression: string, context: Record<string, any> = {}): any {
  try {
    const tokenizer = new Tokenizer(expression);
    const tokens = tokenizer.tokenize();

    const parser = new Parser(tokens);
    const ast = parser.parse();

    const evaluator = new Evaluator(context);
    return evaluator.evaluate(ast);
  } catch (error) {
    console.error('❌ [HUML Evaluator] Failed to evaluate expression:', expression, error);
    return null;
  }
}

/**
 * Evaluate a value that might contain a HUML expression
 * If the value is a string wrapped in {{ }}, it evaluates it
 * Otherwise returns the value as-is
 */
export function evaluateValue(value: any, context: Record<string, any> = {}): any {
  if (typeof value !== 'string') {
    return value;
  }

  const trimmed = value.trim();

  // Check if it has {{}} wrapper
  if (trimmed.startsWith('{{') && trimmed.endsWith('}}')) {
    const expression = trimmed.slice(2, -2).trim();
    return evaluateExpression(expression, context);
  }

  return value;
}

/**
 * Check if a value contains a HUML expression
 */
export function hasExpression(value: any): boolean {
  if (typeof value !== 'string') {
    return false;
  }
  const trimmed = value.trim();
  return trimmed.startsWith('{{') && trimmed.endsWith('}}');
}

/**
 * Alternative export name for backwards compatibility
 */
export const evaluateCEL = evaluateExpression;
