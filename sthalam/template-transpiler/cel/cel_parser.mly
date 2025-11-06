%{
open Cel_types
%}

/* Tokens - Literals */
%token <int64> INT
%token <int64> UINT
%token <float> FLOAT
%token <string> STRING
%token <bytes> BYTES
%token <string> IDENT
%token TRUE FALSE NULL

/* Tokens - Operators */
%token EQ NE LT GT LE GE IN
%token PLUS MINUS STAR SLASH PERCENT
%token AND OR NOT

/* Tokens - Symbols */
%token LPAREN RPAREN
%token LBRACK RBRACK
%token LBRACE RBRACE
%token DOT COMMA
%token QUESTION COLON
%token ARROW
%token EOF

/* Precedence and associativity (lowest to highest) */
%right QUESTION COLON        /* Ternary */
%left OR                     /* Logical OR */
%left AND                    /* Logical AND */
%left EQ NE                  /* Equality */
%left LT GT LE GE IN         /* Relational */
%left PLUS MINUS             /* Addition/Subtraction */
%left STAR SLASH PERCENT     /* Multiplication/Division */
%right NOT                   /* Unary NOT */
%nonassoc UMINUS             /* Unary minus */
%left DOT LBRACK             /* Member access */

%start <Cel_types.expr> main

%%

main:
  | e = expr EOF
    { e }
  ;

expr:
  /* Ternary conditional: cond ? true : false */
  | cond = expr QUESTION if_true = expr COLON if_false = expr
    { Ternary { condition = cond; if_true; if_false } }

  /* Logical OR */
  | left = expr OR right = expr
    { Binary { op = OpOr; left; right } }

  /* Logical AND */
  | left = expr AND right = expr
    { Binary { op = OpAnd; left; right } }

  /* Equality */
  | left = expr EQ right = expr
    { Binary { op = OpEq; left; right } }
  | left = expr NE right = expr
    { Binary { op = OpNe; left; right } }

  /* Relational */
  | left = expr LT right = expr
    { Binary { op = OpLt; left; right } }
  | left = expr GT right = expr
    { Binary { op = OpGt; left; right } }
  | left = expr LE right = expr
    { Binary { op = OpLe; left; right } }
  | left = expr GE right = expr
    { Binary { op = OpGe; left; right } }
  | left = expr IN right = expr
    { Binary { op = OpIn; left; right } }

  /* Additive */
  | left = expr PLUS right = expr
    { Binary { op = OpAdd; left; right } }
  | left = expr MINUS right = expr
    { Binary { op = OpSub; left; right } }

  /* Multiplicative */
  | left = expr STAR right = expr
    { Binary { op = OpMul; left; right } }
  | left = expr SLASH right = expr
    { Binary { op = OpDiv; left; right } }
  | left = expr PERCENT right = expr
    { Binary { op = OpMod; left; right } }

  /* Unary */
  | NOT operand = expr
    { Unary { op = OpNot; operand } }
  | MINUS operand = expr %prec UMINUS
    { Unary { op = OpNeg; operand } }

  /* Member access: obj.field */
  | obj = expr DOT field = IDENT
    { Member { object_ = obj; field } }

  /* Method call: obj.method(args) */
  | obj = expr DOT method_ = IDENT LPAREN args = separated_list(COMMA, expr) RPAREN
    { MethodCall { object_ = obj; method_; args } }

  /* Index access: obj[index] */
  | obj = expr LBRACK index = expr RBRACK
    { Index { object_ = obj; index } }

  /* Function call: func(args) */
  | func = IDENT LPAREN args = separated_list(COMMA, expr) RPAREN
    { Call { function_ = func; args } }

  /* Lambda: param => body */
  | param = IDENT ARROW body = expr
    { Lambda { param; body } }

  /* Primary expressions */
  | e = primary
    { e }
  ;

primary:
  /* Parenthesized expression */
  | LPAREN e = expr RPAREN
    { e }

  /* Literals - Boolean */
  | TRUE
    { Literal (VBool true) }
  | FALSE
    { Literal (VBool false) }

  /* Literals - Null */
  | NULL
    { Literal VNull }

  /* Literals - Numeric */
  | i = INT
    { Literal (VInt i) }
  | u = UINT
    { Literal (VUint u) }
  | f = FLOAT
    { Literal (VFloat f) }

  /* Literals - String */
  | s = STRING
    { Literal (VString s) }

  /* Literals - Bytes */
  | b = BYTES
    { Literal (VBytes b) }

  /* List literal: [e1, e2, ..., eN] or [] */
  | LBRACK items = separated_list(COMMA, expr) RBRACK
    { ListLit items }

  /* Map literal: {k1: v1, k2: v2, ...} or {} */
  | LBRACE entries = separated_list(COMMA, map_entry) RBRACE
    { MapLit entries }

  /* Identifier */
  | id = IDENT
    { Ident id }
  ;

map_entry:
  | key = expr COLON value = expr
    { (key, value) }
  ;

%%
