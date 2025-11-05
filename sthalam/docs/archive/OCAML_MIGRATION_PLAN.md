# OCaml Evaluator Migration Plan

**From**: Custom HUML Expression Language
**To**: CEL (Common Expression Language) with Custom Evaluation

**Date**: 2025-11-04
**Status**: Planning

---

## Current State

The OCaml evaluator in `/huml-evaluator-ocaml/eval-lib/` currently implements a **custom expression language** with human-readable syntax:

### Current Syntax (HUML Expressions)
```huml
# Logical operators
draftTitle is not empty and not isPublishing

# Comparison operators
count greater than 5
count at least 10
value equals "test"

# State checks
array is empty
item in list

# Function calls
size of posts
filter of posts
```

### Current Implementation
- **expr_types.ml** - AST types (VNull, VBool, VInt, VArray, etc.)
- **expr_lexer.mll** - Tokenizer (ocamllex)
- **expr_parser.mly** - Parser (Menhir) with human-readable keywords
- **expr_eval.ml** - Evaluator

---

## Target State

Implement **CEL (Common Expression Language)** with custom evaluation logic for Loro integration.

### Target Syntax (CEL)
```cel
// Logical operators
draftTitle != "" && !isPublishing

// Comparison operators
count > 5
count >= 10
value == "test"

// Collection operations
posts.size()
posts.filter(p, p.author == user)

// Null safety
value != null ? value : "default"
```

### Why CEL?
1. **Standardized** - Used by Kubernetes, Firebase, Google Cloud
2. **Documented** - Complete spec at https://github.com/google/cel-spec
3. **Transferable** - Skills users learn work elsewhere
4. **Professional** - Industry-standard expression language

### Why Keep OCaml?
1. **Performance** - 40x faster than JavaScript (already measured)
2. **Security** - Full control over sandboxing
3. **Customization** - Loro integration, lazy loading
4. **Bundle Size** - ~60KB WASM vs 150KB+ for JS libraries

---

## Migration Strategy

### Phase 1: Extend Parser (Backward Compatible)

Add CEL syntax **alongside** existing HUML syntax:

```ocaml
(* Support both styles during transition *)

(* HUML style *)
| left = expr IS NOT EMPTY
    { StateCheck { op = IsNotEmpty; left; right = None } }

(* CEL style *)
| left = expr NE_SYM right = expr  (* != *)
    { BinaryOp { op = Ne; left; right } }

| left = expr AND_SYM AND_SYM right = expr  (* && *)
    { BinaryOp { op = And; left; right } }
```

**Files to update**:
- `expr_lexer.mll` - Add CEL tokens (`!=`, `&&`, `||`, etc.)
- `expr_parser.mly` - Add CEL syntax rules
- Keep existing HUML rules for backward compatibility

### Phase 2: Implement CEL Standard Library

Add CEL functions to `expr_eval.ml`:

```ocaml
let cel_stdlib = [
  (* String functions *)
  ("contains", fun args ctx ->
    match args with
    | [VString s; VString sub] -> VBool (String.contains_substring s sub)
    | _ -> raise (Type_error "contains(string, string)"));

  ("startsWith", fun args ctx ->
    match args with
    | [VString s; VString prefix] -> VBool (String.starts_with s prefix)
    | _ -> raise (Type_error "startsWith(string, string)"));

  ("split", fun args ctx ->
    match args with
    | [VString s; VString delim] ->
        let parts = String.split_on_char (String.get delim 0) s in
        VArray (List.map (fun p -> VString p) parts)
    | _ -> raise (Type_error "split(string, string)"));

  (* Collection functions *)
  ("filter", fun args ctx ->
    match args with
    | [VArray items; lambda] ->
        (* Custom: Check if items is Loro collection *)
        let filtered = filter_with_predicate items lambda ctx in
        VArray filtered
    | _ -> raise (Type_error "filter(array, lambda)"));

  ("map", fun args ctx ->
    match args with
    | [VArray items; lambda] ->
        let mapped = List.map (fun item -> eval_lambda lambda item ctx) items in
        VArray mapped
    | _ -> raise (Type_error "map(array, lambda)"));

  ("size", fun args ctx ->
    match args with
    | [VArray arr] -> VInt (List.length arr)
    | [VString s] -> VInt (String.length s)
    | _ -> raise (Type_error "size(array|string)"));

  (* Type conversions *)
  ("int", fun args ctx ->
    match args with
    | [VString s] -> (try VInt (int_of_string s) with _ -> raise (Eval_error "Invalid int"))
    | [VFloat f] -> VInt (int_of_float f)
    | [v] -> raise (Type_error "int(string|float)"));

  ("string", fun args ctx ->
    match args with
    | [v] -> VString (value_to_string v)
    | _ -> raise (Type_error "string(any)"));

  (* Date/time *)
  ("now", fun args ctx ->
    match args with
    | [] -> VFloat (Unix.gettimeofday ())
    | _ -> raise (Type_error "now()"));
]
```

### Phase 3: Lambda Support

Add lambda expressions for CEL's filter/map syntax:

```ocaml
(* In expr_types.ml *)
type expr =
  | ...
  | Lambda of { param: string; body: expr }  (* p => p.author == user *)

(* In expr_parser.mly *)
expr:
  | ...
  | param = ID ARROW body = expr
      { Lambda { param; body } }

(* In expr_eval.ml *)
let eval_lambda lambda arg_value ctx =
  match lambda with
  | Lambda { param; body } ->
      let ctx' = add_to_context ctx param arg_value in
      eval body ctx'
  | _ -> raise (Eval_error "Expected lambda")
```

### Phase 4: Loro Integration (Custom Evaluation)

Detect Loro collections and use lazy loading:

```ocaml
(* In expr_eval.ml *)

(* Add Loro value type *)
type value =
  | ...
  | VLoroCollection of { doc: loro_doc; path: string; length: int }

(* Custom filter for Loro collections *)
let eval_filter collection predicate ctx =
  match collection with
  | VLoroCollection { doc; path; length } ->
      (* Don't load all items! Create lazy iterator *)
      let iter = loro_create_iterator doc path in
      let filtered = lazy_filter iter predicate ctx in
      VLoroCollection { doc; path = filtered_path; length = count_filtered filtered }

  | VArray items ->
      (* Regular array: filter in memory *)
      let filtered = List.filter (fun item ->
        match eval_lambda predicate item ctx with
        | VBool true -> true
        | _ -> false
      ) items in
      VArray filtered

  | _ -> raise (Type_error "filter requires array or collection")
```

### Phase 5: Security Sandboxing

Enforce security restrictions:

```ocaml
(* In expr_eval.ml *)

(* Blocked function names *)
let blocked_functions = [
  "eval"; "import"; "require"; "fetch";
  "XMLHttpRequest"; "localStorage"; "sessionStorage";
]

(* Validate function calls *)
let eval_call callee args ctx =
  (* Check if blocked *)
  if List.mem callee blocked_functions then
    raise (Security_error ("Function not allowed: " ^ callee));

  (* Check if in stdlib *)
  match List.assoc_opt callee cel_stdlib with
  | Some fn -> fn args ctx
  | None -> raise (Eval_error ("Unknown function: " ^ callee))

(* Expression timeout *)
let eval_with_timeout expr ctx timeout_ms =
  let start_time = Unix.gettimeofday () in
  let timeout_s = float_of_int timeout_ms /. 1000.0 in

  let rec eval_checked e =
    (* Check timeout before each operation *)
    if Unix.gettimeofday () -. start_time > timeout_s then
      raise (Timeout_error "Expression evaluation timeout");
    eval e ctx
  in

  eval_checked expr
```

---

## Implementation Checklist

### Phase 1: Parser Extension
- [ ] Add CEL tokens to `expr_lexer.mll`
  - [ ] `!=` (NE_SYM)
  - [ ] `&&` (AND_SYM)
  - [ ] `||` (OR_SYM)
  - [ ] `!` (NOT_SYM)
  - [ ] `=>` (ARROW) for lambdas
- [ ] Add CEL syntax to `expr_parser.mly`
  - [ ] Binary operators: `!=`, `&&`, `||`
  - [ ] Unary operators: `!`
  - [ ] Method call syntax: `expr.method(args)`
  - [ ] Lambda syntax: `param => body`
- [ ] Keep existing HUML syntax (backward compatible)
- [ ] Test both syntaxes work

### Phase 2: CEL Standard Library
- [ ] Implement string functions
  - [ ] `contains(string, substring)`
  - [ ] `startsWith(string, prefix)`
  - [ ] `endsWith(string, suffix)`
  - [ ] `trim(string)`
  - [ ] `toLowerCase(string)`
  - [ ] `toUpperCase(string)`
  - [ ] `split(string, delimiter)`
- [ ] Implement collection functions
  - [ ] `size(array|string)`
  - [ ] `filter(array, lambda)`
  - [ ] `map(array, lambda)`
  - [ ] `exists(array, lambda)`
  - [ ] `all(array, lambda)`
- [ ] Implement type conversions
  - [ ] `int(value)`
  - [ ] `double(value)`
  - [ ] `string(value)`
- [ ] Implement date/time
  - [ ] `now()`
  - [ ] `timestamp(string)`
  - [ ] `duration(string)`

### Phase 3: Lambda Support
- [ ] Add `Lambda` to AST types
- [ ] Parse lambda syntax `param => body`
- [ ] Evaluate lambda with parameter binding
- [ ] Test nested lambdas

### Phase 4: Loro Integration
- [ ] Add `VLoroCollection` value type
- [ ] Detect Loro collections in context
- [ ] Implement lazy filtering
- [ ] Implement lazy mapping
- [ ] Test with large collections (10,000+ items)
- [ ] Benchmark performance vs loading all

### Phase 5: Security
- [ ] Block dangerous function names
- [ ] Implement expression timeout (100ms)
- [ ] Validate no access to globals
- [ ] Test injection attacks
- [ ] Audit CEL stdlib for safety

### Phase 6: Testing
- [ ] Unit tests for all CEL functions
- [ ] Parser tests for CEL syntax
- [ ] Security tests (blocked functions, timeouts)
- [ ] Performance benchmarks
- [ ] Integration tests with Loro
- [ ] Compare with STHALAM_DSL.md examples

### Phase 7: Documentation
- [ ] Update README with CEL syntax
- [ ] Document custom Loro evaluation
- [ ] Add security guidelines
- [ ] Provide migration guide from HUML syntax

---

## File Structure After Migration

```
huml-evaluator-ocaml/
├── eval-lib/
│   ├── expr_types.ml        # AST types (add Lambda, VLoroCollection)
│   ├── expr_lexer.mll       # Lexer (add CEL tokens)
│   ├── expr_parser.mly      # Parser (add CEL syntax)
│   ├── expr_eval.ml         # Evaluator (add CEL stdlib, Loro logic)
│   ├── cel_stdlib.ml        # NEW: CEL standard library
│   ├── loro_integration.ml  # NEW: Loro-specific evaluation
│   └── security.ml          # NEW: Security validation
├── eval-bin/
│   └── main.ml              # CLI tool
├── test/
│   ├── test_cel_syntax.ml   # NEW: CEL syntax tests
│   ├── test_cel_stdlib.ml   # NEW: CEL function tests
│   ├── test_loro.ml         # NEW: Loro integration tests
│   └── test_security.ml     # NEW: Security tests
└── dune-project
```

---

## Example: Before vs After

### Before (HUML Expressions)
```huml
computed::
  viewer:
    filteredPosts:
      type: collection
      expr: """
        posts
          where author equals currentUser
          and status is not empty
      """
      depends::
        - content.posts
        - viewerState.currentUser
```

### After (CEL with OCaml Evaluation)
```huml
computed::
  viewer:
    filteredPosts:
      type: collection
      expr: """
        posts.filter(p,
          p.author == currentUser &&
          p.status != ""
        )
      """
      depends::
        - content.posts
        - viewerState.currentUser
```

**Same OCaml evaluator** - just different syntax parsed!

**Custom magic**: When `posts` is a Loro collection with 10,000 items, OCaml doesn't load all items - it creates a lazy filtered view. User writes standard CEL, gets Loro performance! 🎯

---

## Timeline Estimate

- **Phase 1** (Parser): 2-3 days
- **Phase 2** (CEL Stdlib): 3-4 days
- **Phase 3** (Lambdas): 1-2 days
- **Phase 4** (Loro): 2-3 days
- **Phase 5** (Security): 1-2 days
- **Phase 6** (Testing): 2-3 days
- **Phase 7** (Docs): 1 day

**Total**: ~2-3 weeks of focused work

---

## Next Steps

1. Review this plan with team
2. Set up development branch: `feature/cel-migration`
3. Start with Phase 1 (parser extension)
4. Test incrementally after each phase
5. Keep `main` branch stable with HUML syntax

---

## Questions to Resolve

1. **Backward compatibility**: Keep HUML syntax forever, or deprecate after migration period?
2. **Loro bindings**: How to interface with Loro from OCaml? FFI? Separate service?
3. **WASM size**: Will CEL stdlib increase bundle size significantly?
4. **Date/time**: Use Unix timestamps or custom date type?
5. **Type system**: Implement CEL's type checking or keep dynamic?

---

**End of Migration Plan**
