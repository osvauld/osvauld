# Implementation: OCaml React Reactivity + Runtime Type Validation

**Status:** Implementation Plan
**Target:** Sthalam template-transpiler WASM module
**Breaking Changes:** Yes - all templates must be updated

---

## Overview

Implement two major features for HUML templates:

1. **Fine-grained Reactivity:** Use OCaml React FRP library for smart dependency tracking
2. **Runtime Type Validation:** Define schemas in HUML, validate at runtime

**Goals:**
- Move computation reactivity from Svelte to OCaml WASM
- Only recompute values when dependencies actually change
- Validate state updates against type schemas
- Catch type errors early (template load time, not user interaction time)
- Maintain clear separation: WASM handles data, Svelte handles UI

---

## Architecture Overview

```
┌──────────────────────────────────────────────┐
│          HUML Template                       │
│  • Define types (schemas)                    │
│  • Define state with type annotations        │
│  • Define computed values with dependencies  │
└──────────────┬───────────────────────────────┘
               │ (parse once at init)
               ↓
┌──────────────────────────────────────────────┐
│      OCaml WASM (Stateful + Typed)          │
│  • OCaml React signals                       │
│  • Type schema registry                      │
│  • Smart recomputation                       │
│  • Runtime type validation                   │
└──────────────┬───────────────────────────────┘
               │ (emit typed, validated values)
               ↓
┌──────────────────────────────────────────────┐
│         Svelte Component (UI)                │
│  • Receive validated values                  │
│  • Render UI                                 │
│  • Handle user interactions                  │
└──────────────────────────────────────────────┘
```

---

## Part 1: Reactive System with Explicit Dependencies

### 1.1 HUML Template Syntax

**State Definition (unchanged):**
```yaml
documents::
  publisherState::
    counter::
      type: "number"
      initial: 0
    message::
      type: "string"
      initial: "Hello"
```

**Computed Values (NEW - explicit dependencies required):**
```yaml
publisherComputed::
  counterDouble::
    expression: "${ counter * 2 }"
    depends: ["counter"]

  isPositive::
    expression: "${ counter > 0 }"
    depends: ["counter"]

  messageLength::
    expression: "${ size(message) }"
    depends: ["message"]

  combined::
    expression: "${ counterDouble + messageLength }"
    depends: ["counterDouble", "messageLength"]
```

**Key Points:**
- ✅ REQUIRED: All computed values must use `{ expression, depends }` format
- ❌ REMOVED: Old string format `computedValue: "${ expr }"` no longer supported
- Dependencies are explicit array of state/computed value names
- Dependencies can reference other computed values (nested deps)

---

## Part 2: Runtime Type Validation

### 2.1 HUML Type Definitions

**Define types in HUML:**
```yaml
types::
  Post::
    id: "string"
    title: "string"
    content: "string"
    author: "string"
    timestamp: "number"

  Comment::
    id: "string"
    text: "string"
    author: "string"
    timestamp: "number"
```

**Use types in state:**
```yaml
documents::
  collaborativeState::
    posts::
      type: "array"
      schema: "Post"      # NEW: Reference to type definition
      initial: []

    comments::
      type: "array"
      schema: "Comment"
      initial: []
```

**Type-checked computed values:**
```yaml
publisherComputed::
  firstPost::
    expression: "${ posts[0] }"
    depends: ["posts"]
    returnType: "Post"   # NEW: Validate result type

  postTitles::
    expression: "${ map(posts, p => p.title) }"
    depends: ["posts"]
    returnType: "string[]"  # Array of strings
```

---

## Phase 1: Setup & Dependencies

### 1.1 Install OCaml React

```bash
cd /home/abe/osvauld/sthalam/template-transpiler
opam install react
```

### 1.2 Update dune-project

Add `react` to dependencies:

```lisp
(package
  (name template-transpiler)
  (depends
    ocaml
    huml
    menhir
    js_of_ocaml
    js_of_ocaml-ppx
    logs
    react          ; NEW
    yojson
    (alcotest :with-test)))
```

### 1.3 Update eval-bin/dune

Add `react` to libraries:

```lisp
(executable
 (name cel_wasm)
 (modules logging cel_wasm reactive_state type_schema)  ; Add new modules
 (libraries cel js_of_ocaml js_of_ocaml-ppx logs logs.browser react)  ; Add react
 (preprocess
  (pps js_of_ocaml-ppx))
 (modes wasm))
```

### 1.4 Verify Build

```bash
dune build
```

If React library causes issues with WASM mode, this is discovered early.

---

## Phase 2: Type Schema System

### 2.1 Create type_schema.ml

**Purpose:** Parse and validate types from HUML templates

```ocaml
(** Type Schema System for Runtime Validation *)

open Cel.Cel_types

(** Type definitions *)
type primitive_type =
  | TString
  | TNumber
  | TBoolean
  | TNull

type field_schema = {
  name: string;
  field_type: type_schema;
  required: bool;
}

and type_schema =
  | Primitive of primitive_type
  | Array of type_schema
  | Object of field_schema list
  | Named of string  (* Reference to named type *)

(** Type registry *)
let type_registry : (string, type_schema) Hashtbl.t = Hashtbl.create 16

(** Register a named type *)
let register_type (name : string) (schema : type_schema) : unit =
  Hashtbl.add type_registry name schema;
  Logs.debug (fun m -> m "Registered type: %s" name)

(** Look up a named type *)
let lookup_type (name : string) : type_schema option =
  Hashtbl.find_opt type_registry name

(** Parse type definition from HUML *)
let rec parse_type_schema (type_js : Js_of_ocaml.Js.Unsafe.any) : type_schema =
  let open Js_of_ocaml in
  let type_str = Js.to_string (Js.Unsafe.coerce type_js) in

  match type_str with
  | "string" -> Primitive TString
  | "number" -> Primitive TNumber
  | "boolean" -> Primitive TBoolean
  | "null" -> Primitive TNull
  | s when String.ends_with ~suffix:"[]" s ->
      (* Array type: "Post[]" *)
      let elem_type = String.sub s 0 (String.length s - 2) in
      Array (Named elem_type)
  | s -> Named s  (* Named type reference *)

(** Parse object type definition *)
let parse_object_type (obj_js : Js_of_ocaml.Js.Unsafe.any) : type_schema =
  let open Js_of_ocaml in
  let keys = Js.object_keys obj_js in
  let fields = ref [] in

  for i = 0 to keys##.length - 1 do
    match Js.Optdef.to_option (Js.array_get keys i) with
    | Some key_js ->
        let field_name = Js.to_string key_js in
        let field_type_js = Js.Unsafe.get obj_js field_name in
        let field_schema = parse_type_schema field_type_js in

        fields := {
          name = field_name;
          field_type = field_schema;
          required = true;  (* All fields required by default *)
        } :: !fields
    | None -> ()
  done;

  Object (List.rev !fields)

(** Load types from HUML template *)
let load_types_from_template (template_js : Js_of_ocaml.Js.Unsafe.any) : unit =
  let open Js_of_ocaml in

  (* Check if template has types:: section *)
  let has_types = Js.to_bool (Js.Unsafe.get template_js "hasOwnProperty" "types") in
  if not has_types then begin
    Logs.info (fun m -> m "No types:: section in template");
    ()
  end else begin
    let types_obj = Js.Unsafe.get template_js "types" in
    let type_names = Js.object_keys types_obj in

    for i = 0 to type_names##.length - 1 do
      match Js.Optdef.to_option (Js.array_get type_names i) with
      | Some name_js ->
          let type_name = Js.to_string name_js in
          let type_def = Js.Unsafe.get types_obj type_name in
          let schema = parse_object_type type_def in

          register_type type_name schema;
          Logs.info (fun m -> m "Loaded type: %s" type_name)
      | None -> ()
    done
  end

(** Validate value against schema *)
let rec validate_value (value : value) (schema : type_schema) : (unit, string) result =
  match schema, value with
  | Primitive TString, VString _ -> Ok ()
  | Primitive TNumber, VInt _ -> Ok ()
  | Primitive TNumber, VFloat _ -> Ok ()
  | Primitive TBoolean, VBool _ -> Ok ()
  | Primitive TNull, VNull -> Ok ()

  | Array elem_schema, VList items ->
      (* Validate all array elements *)
      List.fold_left (fun acc item ->
        match acc with
        | Error _ -> acc
        | Ok () -> validate_value item elem_schema
      ) (Ok ()) items

  | Object fields, VMap pairs ->
      (* Validate all required fields *)
      List.fold_left (fun acc field ->
        match acc with
        | Error _ -> acc
        | Ok () ->
            (* Find field in value *)
            let field_value = List.find_opt (fun (k, _) ->
              match k with
              | VString name -> name = field.name
              | _ -> false
            ) pairs in

            match field_value with
            | Some (_, v) -> validate_value v field.field_type
            | None when field.required ->
                Error (Printf.sprintf "Missing required field: %s" field.name)
            | None -> Ok ()
      ) (Ok ()) fields

  | Named type_name, _ ->
      (* Resolve named type and validate *)
      begin match lookup_type type_name with
      | Some resolved_schema -> validate_value value resolved_schema
      | None -> Error (Printf.sprintf "Unknown type: %s" type_name)
      end

  | _ -> Error "Type mismatch"

(** Validate array of values *)
let validate_array (values : value list) (elem_schema_name : string) : (unit, string) result =
  match lookup_type elem_schema_name with
  | Some elem_schema ->
      List.fold_left (fun acc item ->
        match acc with
        | Error _ -> acc
        | Ok () -> validate_value item elem_schema
      ) (Ok ()) values
  | None -> Error (Printf.sprintf "Unknown schema: %s" elem_schema_name)
```

---

## Phase 3: Reactive State with Type Validation

### 3.1 Create reactive_state.ml

**Purpose:** OCaml React signals with runtime type validation

```ocaml
(** Reactive State Management using OCaml React FRP + Type Validation *)

open React
open Cel.Cel_types
open Type_schema

(** Signal types *)
type signal_info = {
  signal: value React.signal;
  setter: (value -> unit) option;
  dependencies: string list;
  expression: string option;
  schema: type_schema option;  (* Optional type schema *)
  return_type: string option;   (* Expected return type name *)
}

(** Global signal registry *)
let signals : (string, signal_info) Hashtbl.t = Hashtbl.create 32

(** Initialize source signal with optional schema validation *)
let create_source_signal
    (name : string)
    (initial_value : value)
    (schema_name : string option) : unit =

  (* Validate initial value against schema if provided *)
  let schema_opt = match schema_name with
    | Some sn -> Type_schema.lookup_type sn
    | None -> None
  in

  (match schema_opt with
  | Some schema ->
      begin match Type_schema.validate_value initial_value schema with
      | Ok () ->
          Logs.debug (fun m -> m "Initial value validated for %s" name)
      | Error msg ->
          Logs.err (fun m -> m "Initial value validation failed for %s: %s" name msg)
      end
  | None -> ());

  let signal, setter = S.create ~eq:(=) initial_value in
  let info = {
    signal;
    setter = Some setter;
    dependencies = [];
    expression = None;
    schema = schema_opt;
    return_type = schema_name;
  } in
  Hashtbl.add signals name info;
  Logs.debug (fun m -> m "Created source signal: %s" name)

(** Create computed signal with type validation *)
let create_computed_signal
    (name : string)
    (expr_str : string)
    (dep_names : string list)
    (return_type_name : string option) : unit =

  (* Get dependency signals *)
  let dep_signals = List.map (fun dep_name ->
    match Hashtbl.find_opt signals dep_name with
    | Some info -> info.signal
    | None -> failwith (Printf.sprintf "Unknown dependency: %s" dep_name)
  ) dep_names in

  (* Parse CEL expression once *)
  let expr = Cel.Cel_parser.parse_string expr_str in

  (* Lookup return type schema *)
  let return_schema_opt = match return_type_name with
    | Some rtn -> Type_schema.lookup_type rtn
    | None -> None
  in

  (* Create evaluation function with type validation *)
  let eval_and_validate context_pairs =
    let context = VMap context_pairs in
    let result = Cel.Cel_eval.eval expr context in

    (* Validate result type if schema provided *)
    (match return_schema_opt with
    | Some schema ->
        begin match Type_schema.validate_value result schema with
        | Ok () -> ()
        | Error msg ->
            Logs.warn (fun m -> m "Computed %s type validation failed: %s" name msg)
        end
    | None -> ());

    result
  in

  (* Create computed signal *)
  let computed_signal = match dep_signals with
    | [] ->
        (* No dependencies *)
        let result = eval_and_validate [] in
        S.const result
    | [single_dep] ->
        (* Single dependency *)
        S.map (fun dep_value ->
          let pairs = [VString (List.hd dep_names), dep_value] in
          eval_and_validate pairs
        ) single_dep
    | multiple_deps ->
        (* Multiple dependencies *)
        S.l (fun dep_values ->
          let pairs = List.map2 (fun name value ->
            (VString name, value)
          ) dep_names dep_values in
          eval_and_validate pairs
        ) multiple_deps
  in

  let info = {
    signal = computed_signal;
    setter = None;
    dependencies = dep_names;
    expression = Some expr_str;
    schema = return_schema_opt;
    return_type = return_type_name;
  } in
  Hashtbl.add signals name info;
  Logs.debug (fun m -> m "Created computed signal: %s (deps: %s)"
    name (String.concat ", " dep_names))

(** Update signal with type validation *)
let update_signal (name : string) (new_value : value) : unit =
  match Hashtbl.find_opt signals name with
  | Some { setter = Some set_fn; schema = Some schema; _ } ->
      (* Validate before updating *)
      begin match Type_schema.validate_value new_value schema with
      | Ok () ->
          set_fn new_value;
          Logs.debug (fun m -> m "Updated signal: %s (validated)" name)
      | Error msg ->
          Logs.err (fun m -> m "Update rejected for %s: %s" name msg)
      end

  | Some { setter = Some set_fn; schema = None; _ } ->
      (* No schema - update directly *)
      set_fn new_value;
      Logs.debug (fun m -> m "Updated signal: %s (no validation)" name)

  | Some { setter = None; _ } ->
      Logs.warn (fun m -> m "Cannot update computed signal: %s" name)

  | None ->
      Logs.warn (fun m -> m "Unknown signal: %s" name)

(** Get current value *)
let get_signal_value (name : string) : value option =
  match Hashtbl.find_opt signals name with
  | Some info -> Some (S.value info.signal)
  | None -> None

(** Get all current values *)
let get_all_values () : (string * value) list =
  Hashtbl.fold (fun name info acc ->
    (name, S.value info.signal) :: acc
  ) signals []

(** Clear all signals *)
let clear_signals () : unit =
  Hashtbl.clear signals;
  Logs.info (fun m -> m "Cleared all signals")
```

---

## Phase 4: WASM Integration

### 4.1 Update cel_wasm.ml

Add reactive + typed initialization:

```ocaml
(** Initialize reactive system from HUML template *)
let init_reactive_template (template_js : Js.Unsafe.any) : unit =
  Logging.WasmLog.info (fun m -> m "Initializing reactive template with types");

  (* Clear existing state *)
  Reactive_state.clear_signals ();
  Type_schema.clear_types ();

  (* 1. Load type definitions *)
  Type_schema.load_types_from_template template_js;

  (* 2. Create source signals from publisherState *)
  let pub_state = Js.Unsafe.get template_js "publisherState" in
  let state_keys = Js.object_keys pub_state in

  for i = 0 to state_keys##.length - 1 do
    match Js.Optdef.to_option (Js.array_get state_keys i) with
    | Some key_js ->
        let key = Js.to_string key_js in
        let field_config = Js.Unsafe.get pub_state key in
        let initial_js = Js.Unsafe.get field_config "initial" in
        let initial_value = js_to_value initial_js in

        (* Get optional schema *)
        let schema_name =
          if Js.to_bool (Js.Unsafe.fun_call (Js.Unsafe.get field_config "hasOwnProperty") [|Js.Unsafe.inject (Js.string "schema")|]) then
            Some (Js.to_string (Js.Unsafe.get field_config "schema"))
          else
            None
        in

        Reactive_state.create_source_signal key initial_value schema_name
    | None -> ()
  done;

  (* 3. Create computed signals from publisherComputed *)
  let pub_computed = Js.Unsafe.get template_js "publisherComputed" in
  let comp_keys = Js.object_keys pub_computed in

  for i = 0 to comp_keys##.length - 1 do
    match Js.Optdef.to_option (Js.array_get comp_keys i) with
    | Some key_js ->
        let key = Js.to_string key_js in
        let computed_config = Js.Unsafe.get pub_computed key in

        (* Require object format with expression and depends *)
        let expr_str = Js.to_string (Js.Unsafe.get computed_config "expression") in
        let deps_arr = Js.Unsafe.get computed_config "depends" in
        let deps = js_array_to_string_list deps_arr in

        (* Get optional return type *)
        let return_type =
          if Js.to_bool (Js.Unsafe.fun_call (Js.Unsafe.get computed_config "hasOwnProperty") [|Js.Unsafe.inject (Js.string "returnType")|]) then
            Some (Js.to_string (Js.Unsafe.get computed_config "returnType"))
          else
            None
        in

        Reactive_state.create_computed_signal key expr_str deps return_type
    | None -> ()
  done;

  Logging.WasmLog.info (fun m -> m "Reactive template initialized")

(** Helper: Convert JS array to string list *)
let js_array_to_string_list (arr : Js.Unsafe.any) : string list =
  let js_arr : Js.js_string Js.t Js.js_array Js.t = Js.Unsafe.coerce arr in
  let length = js_arr##.length in
  let rec loop i acc =
    if i < 0 then acc
    else
      match Js.Optdef.to_option (Js.array_get js_arr i) with
      | Some str_js -> loop (i - 1) (Js.to_string str_js :: acc)
      | None -> loop (i - 1) acc
  in
  loop (length - 1) []

(** Update state (triggers reactive propagation) *)
let update_reactive_state (name : Js.js_string Js.t) (value_js : Js.Unsafe.any) : unit =
  let name_str = Js.to_string name in
  let value = js_to_value value_js in
  Reactive_state.update_signal name_str value

(** Get current value *)
let get_reactive_value (name : Js.js_string Js.t) : Js.Unsafe.any =
  let name_str = Js.to_string name in
  match Reactive_state.get_signal_value name_str with
  | Some value -> value_to_js value
  | None -> Js.Unsafe.inject Js.undefined

(** Get all current values *)
let get_all_reactive_values () : Js.Unsafe.any =
  let all_values = Reactive_state.get_all_values () in
  let obj = Js.Unsafe.obj [||] in
  List.iter (fun (name, value) ->
    Js.Unsafe.set obj (Js.string name) (value_to_js value)
  ) all_values;
  Js.Unsafe.inject obj

(** Export API *)
let () =
  Logging.init ();

  Js.export "CELReactive"
    (object%js
       method initTemplate template = init_reactive_template template
       method updateState name value = update_reactive_state name value
       method getValue name = get_reactive_value name
       method getAllValues = get_all_reactive_values ()
       method setLogLevel level = Logging.set_level level
       method version = Js.string "1.0.0-reactive-typed"
    end)
```

---

## Phase 5: Build & Test

### 5.1 Build WASM

```bash
cd /home/abe/osvauld/sthalam/template-transpiler
dune build
./deploy-wasm.sh
```

### 5.2 Test Basic Reactivity

Create test file: `test-reactive.html`

```html
<!DOCTYPE html>
<html>
<head>
  <title>Reactive Test</title>
</head>
<body>
  <h1>Reactive + Typed CEL Test</h1>
  <div id="output"></div>

  <script type="module">
    import init, { CELReactive } from './cel_wasm.js';

    await init();

    // Define template with types
    const template = {
      types: {
        Post: {
          id: "string",
          title: "string",
          content: "string"
        }
      },
      publisherState: {
        counter: { type: "number", initial: 0 },
        message: { type: "string", initial: "Hello" }
      },
      publisherComputed: {
        counterDouble: {
          expression: "${ counter * 2 }",
          depends: ["counter"]
        },
        messageLength: {
          expression: "${ size(message) }",
          depends: ["message"]
        },
        combined: {
          expression: "${ counterDouble + messageLength }",
          depends: ["counterDouble", "messageLength"]
        }
      }
    };

    // Initialize
    CELReactive.initTemplate(template);

    // Test: Update counter
    console.log("Initial values:", CELReactive.getAllValues());

    CELReactive.updateState("counter", 5);
    console.log("After counter=5:", CELReactive.getAllValues());
    // Should see: counter=5, counterDouble=10, messageLength=5, combined=15

    CELReactive.updateState("message", "Hello World");
    console.log("After message change:", CELReactive.getAllValues());
    // Should see: messageLength=11, combined=21

    document.getElementById('output').textContent =
      JSON.stringify(CELReactive.getAllValues(), null, 2);
  </script>
</body>
</html>
```

---

## Phase 6: Update Example Templates

### 6.1 Update sync_test_simple.huml

Add types and explicit dependencies:

```yaml
name: "Sync Test Simple"
version: "v1.0.0"

types::
  Comment::
    id: "string"
    content: "string"
    author: "string"
    timestamp: "number"

documents::
  publisherState::
    title::
      type: "string"
      initial: "Welcome"

  collaborativeState::
    counter::
      type: "number"
      initial: 0
    newComment::
      type: "string"
      initial: ""
    comments::
      type: "array"
      schema: "Comment"
      initial: []

publisherComputed::
  commentCount::
    expression: "${ size(comments) }"
    depends: ["comments"]
    returnType: "number"

# ... rest of template
```

### 6.2 Update twitter_like_simple.huml

Add Post and Comment types:

```yaml
name: "Twitter-Like Posts (Simple)"
version: "v1.0.0"

types::
  Post::
    id: "string"
    title: "string"
    content: "string"
    author: "string"
    timestamp: "number"

  Comment::
    id: "string"
    text: "string"
    author: "string"
    timestamp: "number"

documents::
  publisherState::
    newPostTitle::
      type: "string"
      initial: ""
    newPostContent::
      type: "string"
      initial: ""

  publisherComputed::
    canPost::
      expression: "${ size(newPostTitle) > 0 && size(newPostContent) > 0 }"
      depends: ["newPostTitle", "newPostContent"]
      returnType: "boolean"

    postTitleLength::
      expression: "${ size(newPostTitle) }"
      depends: ["newPostTitle"]
      returnType: "number"

    postContentLength::
      expression: "${ size(newPostContent) }"
      depends: ["newPostContent"]
      returnType: "number"

    totalPosts::
      expression: "${ size(posts) }"
      depends: ["posts"]
      returnType: "number"

  collaborativeState::
    posts::
      type: "array"
      schema: "Post"
      initial: []
    comments::
      type: "array"
      schema: "Comment"
      initial: []

# ... rest of template
```

---

## Success Criteria

**Phase 1 (Reactivity):**
- ✅ OCaml React compiles in WASM mode
- ✅ Template initialization creates signal graph
- ✅ State update triggers only dependent computed values
- ✅ Performance: < 50% CEL evaluations vs. previous implementation

**Phase 2 (Types):**
- ✅ Types section parsed from HUML
- ✅ Schema validation on state updates
- ✅ Return type validation on computed values
- ✅ Clear error messages for type violations

**Integration:**
- ✅ All examples updated and working
- ✅ Svelte components simplified (less reactivity logic)
- ✅ Documentation complete

---

## Testing Strategy

### Unit Tests

Test each module independently:

**type_schema_test.ml:**
```ocaml
let test_parse_primitive () =
  let schema = Type_schema.parse_type_schema (Js.string "string") in
  assert (schema = Primitive TString)

let test_validate_string () =
  let result = Type_schema.validate_value (VString "hello") (Primitive TString) in
  assert (Result.is_ok result)

let test_validate_object () =
  let schema = Object [
    { name = "id"; field_type = Primitive TString; required = true };
    { name = "count"; field_type = Primitive TNumber; required = true };
  ] in
  let value = VMap [
    (VString "id", VString "123");
    (VString "count", VInt 42L);
  ] in
  let result = Type_schema.validate_value value schema in
  assert (Result.is_ok result)
```

**reactive_state_test.ml:**
```ocaml
let test_source_signal () =
  Reactive_state.create_source_signal "counter" (VInt 0L) None;
  Reactive_state.update_signal "counter" (VInt 5L);
  let value = Reactive_state.get_signal_value "counter" in
  assert (value = Some (VInt 5L))

let test_computed_signal () =
  Reactive_state.create_source_signal "counter" (VInt 5L) None;
  Reactive_state.create_computed_signal "double" "counter * 2" ["counter"] None;
  let value = Reactive_state.get_signal_value "double" in
  assert (value = Some (VInt 10L))
```

### Integration Tests

Test WASM module end-to-end:

```javascript
// test-integration.js
import init, { CELReactive } from './cel_wasm.js';

await init();

// Test reactive updates
const template = { /* ... */ };
CELReactive.initTemplate(template);

assert(CELReactive.getValue("counter") === 0);
CELReactive.updateState("counter", 5);
assert(CELReactive.getValue("counter") === 5);
assert(CELReactive.getValue("counterDouble") === 10);

// Test type validation
try {
  CELReactive.updateState("counter", "not a number");
  assert(false, "Should have rejected string for number field");
} catch (e) {
  // Expected
}
```

---

## Performance Expectations

**Before (Svelte + Stateless WASM):**
- Update counter → Re-evaluate ALL 10 computed values
- 10 CEL evaluations
- Full context conversion (14 values)

**After (React Signals + WASM):**
- Update counter → Re-evaluate ONLY dependent values (counterDouble, combined)
- 2 CEL evaluations (80% reduction!)
- No full context conversion

**Expected Improvements:**
- 50-80% reduction in CEL evaluations per state change
- 60-90% reduction in JS↔WASM data conversion
- Faster UI updates (less computation)

---

## Documentation

Update these files:

1. **HUML_TEMPLATE_GUIDE_ACCURATE.md**
   - Add types:: section documentation
   - Update publisherComputed syntax (explicit deps required)
   - Add type validation examples

2. **Create REACTIVE_ARCHITECTURE.md**
   - Explain OCaml React integration
   - Document signal lifecycle
   - Type system overview

3. **Update example READMEs**
   - Show new template syntax
   - Explain reactivity benefits

---

## Implementation Timeline

**Week 1:**
- Install React
- Create type_schema.ml
- Basic type parsing + validation

**Week 2:**
- Create reactive_state.ml
- Integrate OCaml React
- Signal creation + updates

**Week 3:**
- Update cel_wasm.ml
- JavaScript integration
- Test harness

**Week 4:**
- Update all example templates
- Performance testing
- Documentation

---

## Next Steps After Completion

**Future enhancements (not in this implementation):**

1. **Async Support:** Computed values that return Promises
2. **Caching:** Advanced memoization strategies
3. **Throttling:** Rate-limit expensive computations
4. **Code Generation:** Generate OCaml types from HUML (Option B)
5. **IDE Support:** LSP server for HUML templates

---

## Breaking Changes Summary

**Templates must be updated:**

❌ **OLD (no longer works):**
```yaml
publisherComputed::
  counterDouble: "${ counter * 2 }"
```

✅ **NEW (required):**
```yaml
publisherComputed::
  counterDouble::
    expression: "${ counter * 2 }"
    depends: ["counter"]
```

**All templates in `/docs/examples/` must be updated before this works.**

---

**Ready to implement!**
