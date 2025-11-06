# CEL (Common Expression Language) Implementation

Clean, minimal implementation of CEL parser and evaluator in OCaml.

## Files

- **cel_types.ml** - AST types and value representation
- **cel_lexer.mll** - Lexer/tokenizer (ocamllex)
- **cel_parser.mly** - Parser grammar (Menhir)
- **cel_eval.ml** - Basic expression evaluator
- **dune** - Build configuration

## Features Implemented

### Literals
- `null`
- Booleans: `true`, `false`
- Integers: `42`, `0xFF` (hex), `42u` (unsigned)
- Floats: `3.14`, `1e10`
- Strings: `"hello"`, `'world'`, `"""multiline"""`
- Bytes: `b"data"`
- Lists: `[1, 2, 3]`
- Maps: `{"key": "value"}`

### Operators

**Arithmetic**: `+`, `-`, `*`, `/`, `%`

**Comparison**: `<`, `<=`, `>`, `>=`, `==`, `!=`, `in`

**Logical**: `&&`, `||`, `!`

**Ternary**: `condition ? true_value : false_value`

### Member Access

- Dot notation: `obj.field`
- Index notation: `list[0]`, `map["key"]`

### Lambdas

- Arrow syntax: `x => x * 2`
- Used with filter/map (to be implemented in stdlib)

## Not Yet Implemented

- **Function calls** - See cel_stdlib.ml (to be created)
- **Method calls** - String/list methods
- **Macros** - CEL macros like `has()`, `all()`, `exists()`
- **Type checking** - Dynamic evaluation only

## Build

```bash
cd huml-evaluator-ocaml
dune build
```

## Test

```ocaml
(* In OCaml top-level *)
#require "cel";;
open Cel_types;;
open Cel_eval;;

(* Parse and evaluate *)
let lexbuf = Lexing.from_string "1 + 2 * 3" in
let expr = Cel_parser.main Cel_lexer.token lexbuf in
let result = eval [] expr;;
(* Result: VInt 7L *)
```

## Next Steps

1. Implement CEL standard library (cel_stdlib.ml)
2. Add Loro integration for lazy loading (../extensions/loro_integration.ml)
3. Add custom Sthalam keywords (../extensions/custom_keywords.ml)
4. Compile to JavaScript via js_of_ocaml
5. Add comprehensive tests

## References

- CEL Spec: https://github.com/google/cel-spec
- CEL Language Definition: https://github.com/google/cel-spec/blob/master/doc/langdef.md
- cel-rust: https://github.com/cel-rust/cel-rust (reference implementation)
