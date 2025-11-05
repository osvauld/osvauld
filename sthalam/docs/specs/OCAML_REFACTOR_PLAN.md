# OCaml CEL Evaluator Refactoring Plan

**Status:** 📋 Planning
**Created:** 2025-01-06
**Goal:** Refactor monolithic cel_eval.ml into modular structure + add math/geometry functions for canvas

---

## 🎯 Objectives

1. **Split** monolithic `cel_eval.ml` (600+ lines) into clean modules
2. **Add** 15 math functions (sin, cos, sqrt, etc.) for canvas patterns
3. **Add** 7 geometry functions (distance, lerp, etc.) for interactivity
4. **Improve** code organization and maintainability
5. **Enable** canvas block implementation

---

## 📊 Current State Analysis

### Existing Functions (28 total)

**String Functions (9):**
- contains, startsWith, endsWith, trim
- toLowerCase, toUpperCase, split, replace, substring

**Collection Functions (11):**
- size, filter, map, exists, all
- flatten, unique, slice, exists_one, find, join

**Type Conversion (3):**
- int, double, string

**Date/Time Functions (5):**
- now, timestamp, generateId, duration, format

### Current Structure

```
cel/
├── cel_types.ml       (type definitions)
├── cel_eval.ml        (600+ lines - EVERYTHING!)
│   ├── String_funcs module (lines 184-248)
│   ├── Collection_funcs module (lines 251-377)
│   ├── Type_funcs module (lines 380-405)
│   ├── Time_funcs module (lines 408-478)
│   └── stdlib_functions registry (lines 486-522)
├── cel_lexer.mll      (lexer)
├── cel_parser.mly     (parser)
└── dune               (build config)
```

### Missing for Canvas

**Math Functions (0/15):** ❌
- Trigonometry: sin, cos, tan, asin, acos, atan, atan2
- Power/Root: sqrt, pow, exp, log, log10
- Rounding: floor, ceil, round, abs, sign
- Comparison: min, max
- Random: random
- Constants: pi, e

**Geometry Functions (0/7):** ❌
- distance, lerp, clamp, map_range
- normalize, angle, degrees, radians

---

## 🏗️ Target Structure

```
cel/
├── cel_types.ml              [KEEP AS-IS]
├── cel_eval.ml               [REFACTOR - core eval logic only]
├── cel_lexer.mll            [KEEP AS-IS]
├── cel_parser.mly           [KEEP AS-IS]
├── string_funcs.ml          [NEW - extract from cel_eval.ml]
├── collection_funcs.ml      [NEW - extract from cel_eval.ml]
├── type_funcs.ml            [NEW - extract from cel_eval.ml]
├── time_funcs.ml            [NEW - extract from cel_eval.ml]
├── math_funcs.ml            [NEW - add 15 math functions] 🆕
├── geometry_funcs.ml        [NEW - add 7 geometry functions] 🆕
├── stdlib_registry.ml       [NEW - central function registry]
└── dune                      [UPDATE - add new modules]
```

---

## 📋 Implementation Plan

### Phase 1: Extract String Functions (1-2 hours)

**File:** `cel/string_funcs.ml`

**Functions to extract:**
```ocaml
module String_funcs = struct
  let contains : value list -> context -> lambda_eval -> value
  let starts_with : value list -> context -> lambda_eval -> value
  let ends_with : value list -> context -> lambda_eval -> value
  let trim : value list -> context -> lambda_eval -> value
  let to_lower_case : value list -> context -> lambda_eval -> value
  let to_upper_case : value list -> context -> lambda_eval -> value
  let split : value list -> context -> lambda_eval -> value
  let replace : value list -> context -> lambda_eval -> value
  let substring : value list -> context -> lambda_eval -> value
end
```

**Public interface:**
```ocaml
val get_string_functions : unit -> (string * func) list
```

**Tasks:**
- [ ] Create `string_funcs.ml`
- [ ] Copy String_funcs module from cel_eval.ml
- [ ] Add public interface
- [ ] Update cel_eval.ml to import functions
- [ ] Test all 9 string functions

---

### Phase 2: Extract Collection Functions (1-2 hours)

**File:** `cel/collection_funcs.ml`

**Functions to extract:**
```ocaml
module Collection_funcs = struct
  let size : value list -> context -> lambda_eval -> value
  let filter : value list -> context -> lambda_eval -> value
  let map : value list -> context -> lambda_eval -> value
  let exists : value list -> context -> lambda_eval -> value
  let all : value list -> context -> lambda_eval -> value
  let flatten : value list -> context -> lambda_eval -> value
  let unique : value list -> context -> lambda_eval -> value
  let slice : value list -> context -> lambda_eval -> value
  let exists_one : value list -> context -> lambda_eval -> value
  let find : value list -> context -> lambda_eval -> value
  let join : value list -> context -> lambda_eval -> value
end
```

**Public interface:**
```ocaml
val get_collection_functions : unit -> (string * func) list
```

**Tasks:**
- [ ] Create `collection_funcs.ml`
- [ ] Copy Collection_funcs module from cel_eval.ml
- [ ] Handle lambda_eval dependency (pass as parameter)
- [ ] Add public interface
- [ ] Update cel_eval.ml
- [ ] Test all 11 collection functions

---

### Phase 3: Extract Type & Time Functions (1 hour)

**File:** `cel/type_funcs.ml`

**Functions to extract:**
```ocaml
module Type_funcs = struct
  let to_int : value list -> context -> lambda_eval -> value
  let to_double : value list -> context -> lambda_eval -> value
  let to_string : value list -> context -> lambda_eval -> value
end
```

**File:** `cel/time_funcs.ml`

**Functions to extract:**
```ocaml
module Time_funcs = struct
  let now : value list -> context -> lambda_eval -> value
  let timestamp : value list -> context -> lambda_eval -> value
  let generate_id : value list -> context -> lambda_eval -> value
  let duration : value list -> context -> lambda_eval -> value
  let format : value list -> context -> lambda_eval -> value
end
```

**Tasks:**
- [ ] Create `type_funcs.ml` with 3 functions
- [ ] Create `time_funcs.ml` with 5 functions
- [ ] Add public interfaces
- [ ] Update cel_eval.ml
- [ ] Test all functions

---

### Phase 4: Add Math Functions (3-4 hours) 🆕

**File:** `cel/math_funcs.ml`

**Functions to implement:**

```ocaml
module Math_funcs = struct
  (* Trigonometry *)
  val sin : value list -> context -> lambda_eval -> value      (* sin(x) *)
  val cos : value list -> context -> lambda_eval -> value      (* cos(x) *)
  val tan : value list -> context -> lambda_eval -> value      (* tan(x) *)
  val asin : value list -> context -> lambda_eval -> value     (* asin(x) *)
  val acos : value list -> context -> lambda_eval -> value     (* acos(x) *)
  val atan : value list -> context -> lambda_eval -> value     (* atan(x) *)
  val atan2 : value list -> context -> lambda_eval -> value    (* atan2(y, x) *)

  (* Power & Roots *)
  val sqrt : value list -> context -> lambda_eval -> value     (* sqrt(x) *)
  val pow : value list -> context -> lambda_eval -> value      (* pow(base, exp) *)
  val exp : value list -> context -> lambda_eval -> value      (* exp(x) *)
  val log : value list -> context -> lambda_eval -> value      (* log(x) - natural *)
  val log10 : value list -> context -> lambda_eval -> value    (* log10(x) *)

  (* Rounding *)
  val floor : value list -> context -> lambda_eval -> value    (* floor(x) *)
  val ceil : value list -> context -> lambda_eval -> value     (* ceil(x) *)
  val round : value list -> context -> lambda_eval -> value    (* round(x) *)
  val abs : value list -> context -> lambda_eval -> value      (* abs(x) *)
  val sign : value list -> context -> lambda_eval -> value     (* sign(x) → -1, 0, or 1 *)

  (* Comparison *)
  val min : value list -> context -> lambda_eval -> value      (* min(a, b, ...) *)
  val max : value list -> context -> lambda_eval -> value      (* max(a, b, ...) *)

  (* Random *)
  val random : value list -> context -> lambda_eval -> value   (* random() → [0, 1) *)

  (* Constants *)
  val pi : value list -> context -> lambda_eval -> value       (* pi → 3.14159... *)
  val e : value list -> context -> lambda_eval -> value        (* e → 2.71828... *)
end
```

**Implementation notes:**
- Use OCaml's `Float` module for all operations
- Handle both `VInt` and `VFloat` inputs
- Convert `VInt` to `VFloat` for math operations
- Return `VFloat` for all functions (except sign)

**Example implementation:**
```ocaml
let sin args _ctx _eval = match args with
  | [VFloat x] -> VFloat (Float.sin x)
  | [VInt i] -> VFloat (Float.sin (Int64.to_float i))
  | _ -> raise (Type_error "sin(number)")

let atan2 args _ctx _eval = match args with
  | [VFloat y; VFloat x] -> VFloat (Float.atan2 y x)
  | [VInt iy; VInt ix] ->
      VFloat (Float.atan2 (Int64.to_float iy) (Int64.to_float ix))
  | _ -> raise (Type_error "atan2(number, number)")

let pi _args _ctx _eval = VFloat Float.pi
let e _args _ctx _eval = VFloat Float.e
```

**Tasks:**
- [ ] Create `math_funcs.ml`
- [ ] Implement all 22 functions + constants
- [ ] Add comprehensive tests
- [ ] Test with canvas expressions: `sin(x * 0.2 + time)`
- [ ] Verify atan2 works for spirals: `atan2(y - 50, x - 50)`

---

### Phase 5: Add Geometry Functions (2-3 hours) 🆕

**File:** `cel/geometry_funcs.ml`

**Functions to implement:**

```ocaml
module Geometry_funcs = struct
  val distance : value list -> context -> lambda_eval -> value
  (* distance(x1, y1, x2, y2) → float *)
  (* sqrt((x2-x1)^2 + (y2-y1)^2) *)

  val lerp : value list -> context -> lambda_eval -> value
  (* lerp(a, b, t) → float *)
  (* a + (b - a) * t *)

  val clamp : value list -> context -> lambda_eval -> value
  (* clamp(value, min, max) → float *)
  (* max(min, min(value, max)) *)

  val map_range : value list -> context -> lambda_eval -> value
  (* map_range(value, in_min, in_max, out_min, out_max) → float *)
  (* (value - in_min) / (in_max - in_min) * (out_max - out_min) + out_min *)

  val normalize : value list -> context -> lambda_eval -> value
  (* normalize(x, y) → {x: float, y: float} *)
  (* Returns unit vector *)

  val angle : value list -> context -> lambda_eval -> value
  (* angle(x1, y1, x2, y2) → float *)
  (* atan2(y2 - y1, x2 - x1) *)

  val degrees : value list -> context -> lambda_eval -> value
  (* degrees(radians) → float *)
  (* radians * 180.0 / pi *)

  val radians : value list -> context -> lambda_eval -> value
  (* radians(degrees) → float *)
  (* degrees * pi / 180.0 *)
end
```

**Example implementation:**
```ocaml
let distance args _ctx _eval = match args with
  | [VFloat x1; VFloat y1; VFloat x2; VFloat y2] ->
      let dx = x2 -. x1 in
      let dy = y2 -. y1 in
      VFloat (Float.sqrt (dx *. dx +. dy *. dy))
  | _ -> raise (Type_error "distance(x1, y1, x2, y2)")

let lerp args _ctx _eval = match args with
  | [VFloat a; VFloat b; VFloat t] ->
      VFloat (a +. (b -. a) *. t)
  | _ -> raise (Type_error "lerp(a, b, t)")

let clamp args _ctx _eval = match args with
  | [VFloat v; VFloat min_v; VFloat max_v] ->
      VFloat (Float.max min_v (Float.min v max_v))
  | _ -> raise (Type_error "clamp(value, min, max)")
```

**Tasks:**
- [ ] Create `geometry_funcs.ml`
- [ ] Implement all 8 functions
- [ ] Add tests for each function
- [ ] Test with interactive canvas examples

---

### Phase 6: Central Registry (2-3 hours)

**File:** `cel/stdlib_registry.ml`

**Purpose:** Single source of truth for all standard library functions

**Interface:**
```ocaml
type func = {
  name: string;
  impl: value list -> context -> lambda_eval -> value;
}

val get_all_functions : unit -> func list
val lookup_function : string -> func option
```

**Implementation:**
```ocaml
let get_all_functions () =
  List.concat [
    String_funcs.get_string_functions ();
    Collection_funcs.get_collection_functions ();
    Type_funcs.get_type_functions ();
    Time_funcs.get_time_functions ();
    Math_funcs.get_math_functions ();
    Geometry_funcs.get_geometry_functions ();
  ]

let lookup_function name =
  List.find_opt (fun f -> f.name = name) (get_all_functions ())
```

**Tasks:**
- [ ] Create `stdlib_registry.ml`
- [ ] Collect all function registrations
- [ ] Provide lookup API
- [ ] Update cel_eval.ml to use registry
- [ ] Remove old stdlib_functions list from cel_eval.ml

---

### Phase 7: Update cel_eval.ml (1-2 hours)

**Changes:**

1. **Remove old modules:**
   - Delete String_funcs module (lines 184-248)
   - Delete Collection_funcs module (lines 251-377)
   - Delete Type_funcs module (lines 380-405)
   - Delete Time_funcs module (lines 408-478)
   - Delete stdlib_functions list (lines 486-522)

2. **Add imports:**
   ```ocaml
   open Stdlib_registry
   ```

3. **Update function calls:**
   ```ocaml
   (* OLD *)
   let lookup_function name =
     List.find_opt (fun f -> f.name = name) stdlib_functions

   (* NEW *)
   let lookup_function = Stdlib_registry.lookup_function
   ```

4. **Keep in cel_eval.ml:**
   - Core eval logic (lines 529-593)
   - Binary operators (lines 26-120)
   - Unary operators (lines 122-132)
   - Member access (lines 134-156)
   - Index access (lines 159-178)
   - Helper functions

**Expected result:** cel_eval.ml shrinks from 600+ lines to ~300 lines

**Tasks:**
- [ ] Remove extracted modules
- [ ] Add stdlib_registry import
- [ ] Update function lookup
- [ ] Clean up unused code
- [ ] Run full test suite

---

### Phase 8: Update dune File (15 minutes)

**File:** `cel/dune`

**Current:**
```ocaml
(library
 (name cel)
 (public_name huml-eval.cel)
 (libraries yojson str unix)
 (modules cel_types cel_eval cel_lexer cel_parser))
```

**New:**
```ocaml
(library
 (name cel)
 (public_name huml-eval.cel)
 (libraries yojson str unix)
 (modules
   cel_types
   cel_eval
   cel_lexer
   cel_parser
   string_funcs
   collection_funcs
   type_funcs
   time_funcs
   math_funcs
   geometry_funcs
   stdlib_registry))
```

**Tasks:**
- [ ] Update modules list
- [ ] Rebuild project (`dune build`)
- [ ] Verify no compilation errors

---

### Phase 9: Testing & Validation (2 hours)

**Test existing functions:**
```bash
dune test
```

**Test new math functions:**
```ocaml
(* Test expressions for canvas *)
assert (eval "sin(0)" {} = 0.0);
assert (eval "cos(0)" {} = 1.0);
assert (eval "sqrt(4)" {} = 2.0);
assert (eval "atan2(1, 1)" {} ≈ 0.785398);  (* π/4 *)
assert (eval "pi" {} ≈ 3.14159);

(* Canvas pattern expression *)
let expr = "sin(x * 0.2 + time)" in
let ctx = [("x", VFloat 10.0); ("time", VFloat 0.5)] in
assert (eval expr ctx = sin(10.0 * 0.2 + 0.5));
```

**Test new geometry functions:**
```ocaml
assert (eval "distance(0, 0, 3, 4)" {} = 5.0);
assert (eval "lerp(0, 10, 0.5)" {} = 5.0);
assert (eval "clamp(15, 0, 10)" {} = 10.0);
```

**Integration test with WASM:**
```bash
cd eval-bin
dune build
npm test  # If you have JS tests
```

**Tasks:**
- [ ] Run all existing tests
- [ ] Add tests for 22 new math functions
- [ ] Add tests for 8 new geometry functions
- [ ] Test WASM compilation
- [ ] Test canvas pattern expressions
- [ ] Verify 60 FPS performance maintained

---

## 📦 Deliverables

### New Files (7)
1. ✅ `cel/string_funcs.ml` - 9 string functions
2. ✅ `cel/collection_funcs.ml` - 11 collection functions
3. ✅ `cel/type_funcs.ml` - 3 type conversion functions
4. ✅ `cel/time_funcs.ml` - 5 time functions
5. ✅ `cel/math_funcs.ml` - 22 math functions (NEW)
6. ✅ `cel/geometry_funcs.ml` - 8 geometry functions (NEW)
7. ✅ `cel/stdlib_registry.ml` - Central function registry

### Modified Files (2)
1. ✅ `cel/cel_eval.ml` - Refactored, ~300 lines (was 600+)
2. ✅ `cel/dune` - Updated module list

### Function Count
- **Before:** 28 functions
- **After:** 58 functions (28 + 22 math + 8 geometry)

---

## ⏱️ Time Estimate

| Phase | Task | Estimate |
|-------|------|----------|
| 1 | Extract string functions | 1-2 hours |
| 2 | Extract collection functions | 1-2 hours |
| 3 | Extract type & time functions | 1 hour |
| 4 | Add math functions | 3-4 hours |
| 5 | Add geometry functions | 2-3 hours |
| 6 | Central registry | 2-3 hours |
| 7 | Refactor cel_eval.ml | 1-2 hours |
| 8 | Update dune | 15 min |
| 9 | Testing & validation | 2 hours |
| **TOTAL** | | **13-19 hours** |

---

## ✅ Success Criteria

1. ✅ All 28 existing functions work identically
2. ✅ 22 new math functions implemented and tested
3. ✅ 8 new geometry functions implemented and tested
4. ✅ Canvas expressions work: `sin(x * 0.2 + time)`
5. ✅ Clean modular structure (7 new files)
6. ✅ cel_eval.ml reduced from 600+ to ~300 lines
7. ✅ All tests passing
8. ✅ WASM compilation successful
9. ✅ 60 FPS performance maintained
10. ✅ No external dependencies added

---

## 🚀 Getting Started

```bash
# Navigate to OCaml directory
cd sthalam/frontend/desktop/huml-evaluator-ocaml

# Start with Phase 1
touch cel/string_funcs.ml
# ... extract String_funcs module ...

# Build and test after each phase
dune build
dune test

# Final WASM build
cd eval-bin
dune build
```

---

## 📚 References

- Current: `/cel/cel_eval.ml`
- OCaml Float docs: https://ocaml.org/api/Float.html
- Canvas spec: `/docs/specs/CANVAS_BLOCK_SPEC.md`
- WASM knowledge: `/docs/WASM_TAURI_KNOWLEDGE.md`

---

**Status:** Ready to implement! 🎯
