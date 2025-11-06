(** CEL Evaluator - Basic expression evaluation

    This is a minimal evaluator for CEL expressions.
    Custom extensions (Loro integration, etc.) will be in separate files.

    Standard library functions are now organized in separate modules:
    - String_funcs: 9 string manipulation functions
    - Collection_funcs: 11 collection operations
    - Type_funcs: 3 type conversions
    - Time_funcs: 5 date/time functions
    - Math_funcs: 22 mathematical functions
    - Geometry_funcs: 8 geometric utility functions
*)

open Cel_types

(** Look up identifier in context *)
let lookup_ident (ctx : context) (name : string) : value =
  match List.assoc_opt name ctx with
  | Some v -> v
  | None -> raise (Unknown_identifier name)

(** Evaluate binary operator *)
let eval_binary_op (op : binary_op) (left : value) (right : value) : value =
  match op with
  (* Relational operators *)
  | OpLt -> VBool (compare_values left right < 0)
  | OpLe -> VBool (compare_values left right <= 0)
  | OpGt -> VBool (compare_values left right > 0)
  | OpGe -> VBool (compare_values left right >= 0)
  | OpEq -> VBool (values_equal left right)
  | OpNe -> VBool (not (values_equal left right))

  | OpIn -> (
      (* Check if left is in right (right must be list or map) *)
      match right with
      | VList items -> VBool (List.exists (values_equal left) items)
      | VMap entries ->
          VBool (List.exists (fun (k, _) -> values_equal left k) entries)
      | VString s -> (
          match left with
          | VString sub ->
              (* Check if string contains substring *)
              let contains =
                try
                  let _ = Str.search_forward (Str.regexp_string sub) s 0 in
                  true
                with Not_found -> false
              in
              VBool contains
          | _ -> raise (Type_error "in operator requires string substring"))
      | _ -> raise (Type_error "in operator requires list, map, or string"))

  (* Arithmetic operators *)
  | OpAdd -> (
      match (left, right) with
      | VInt a, VInt b -> VInt (Int64.add a b)
      | VUint a, VUint b -> VUint (Int64.add a b)
      | VFloat a, VFloat b -> VFloat (a +. b)
      | VInt a, VFloat b -> VFloat (Int64.to_float a +. b)
      | VFloat a, VInt b -> VFloat (a +. Int64.to_float b)
      | VString a, VString b -> VString (a ^ b)
      | VList a, VList b -> VList (a @ b)
      | _ ->
          raise
            (Type_error
               (Printf.sprintf "Cannot add %s and %s" (type_name left)
                  (type_name right))))

  | OpSub -> (
      match (left, right) with
      | VInt a, VInt b -> VInt (Int64.sub a b)
      | VUint a, VUint b -> VUint (Int64.sub a b)
      | VFloat a, VFloat b -> VFloat (a -. b)
      | VInt a, VFloat b -> VFloat (Int64.to_float a -. b)
      | VFloat a, VInt b -> VFloat (a -. Int64.to_float b)
      | _ ->
          raise
            (Type_error
               (Printf.sprintf "Cannot subtract %s and %s" (type_name left)
                  (type_name right))))

  | OpMul -> (
      match (left, right) with
      | VInt a, VInt b -> VInt (Int64.mul a b)
      | VUint a, VUint b -> VUint (Int64.mul a b)
      | VFloat a, VFloat b -> VFloat (a *. b)
      | VInt a, VFloat b -> VFloat (Int64.to_float a *. b)
      | VFloat a, VInt b -> VFloat (a *. Int64.to_float b)
      | _ ->
          raise
            (Type_error
               (Printf.sprintf "Cannot multiply %s and %s" (type_name left)
                  (type_name right))))

  | OpDiv -> (
      match (left, right) with
      | VInt a, VInt b when b <> 0L -> VInt (Int64.div a b)
      | VUint a, VUint b when b <> 0L -> VUint (Int64.div a b)
      | VFloat a, VFloat b -> VFloat (a /. b)
      | VInt a, VFloat b -> VFloat (Int64.to_float a /. b)
      | VFloat a, VInt b -> VFloat (a /. Int64.to_float b)
      | _, _ when (match right with VInt 0L | VUint 0L -> true | _ -> false) ->
          raise (Eval_error "Division by zero")
      | _ ->
          raise
            (Type_error
               (Printf.sprintf "Cannot divide %s by %s" (type_name left)
                  (type_name right))))

  | OpMod -> (
      match (left, right) with
      | VInt a, VInt b when b <> 0L -> VInt (Int64.rem a b)
      | VUint a, VUint b when b <> 0L -> VUint (Int64.rem a b)
      | _, _ when (match right with VInt 0L | VUint 0L -> true | _ -> false) ->
          raise (Eval_error "Modulo by zero")
      | _ ->
          raise
            (Type_error
               (Printf.sprintf "Cannot modulo %s by %s" (type_name left)
                  (type_name right))))

  (* Logical operators *)
  | OpAnd -> VBool (is_truthy left && is_truthy right)
  | OpOr -> VBool (is_truthy left || is_truthy right)

(** Evaluate unary operator *)
let eval_unary_op (op : unary_op) (operand : value) : value =
  match op with
  | OpNot -> VBool (not (is_truthy operand))
  | OpNeg -> (
      match operand with
      | VInt i -> VInt (Int64.neg i)
      | VFloat f -> VFloat (-.f)
      | _ ->
          raise
            (Type_error (Printf.sprintf "Cannot negate %s" (type_name operand))))

(** Get member field from object *)
let get_member (obj : value) (field : string) : value =
  match obj with
  | VMap entries -> (
      match
        List.find_opt (fun (k, _) -> values_equal k (VString field)) entries
      with
      | Some (_, v) -> v
      | None -> VNull (* Field not found returns null *))
  | VString s -> (
      (* String properties *)
      match field with
      | "size" -> VInt (Int64.of_int (String.length s))
      | _ -> raise (Eval_error (Printf.sprintf "Unknown string property: %s" field)))
  | VList lst -> (
      (* List properties *)
      match field with
      | "size" -> VInt (Int64.of_int (List.length lst))
      | _ -> raise (Eval_error (Printf.sprintf "Unknown list property: %s" field)))
  | _ ->
      raise
        (Type_error
           (Printf.sprintf "Cannot access field '%s' on %s" field
              (type_name obj)))

(** Get index from collection *)
let get_index (obj : value) (index : value) : value =
  match (obj, index) with
  | VList items, VInt i -> (
      let idx = Int64.to_int i in
      if idx >= 0 && idx < List.length items then List.nth items idx
      else raise (Eval_error "List index out of bounds"))
  | VMap entries, key -> (
      match List.find_opt (fun (k, _) -> values_equal k key) entries with
      | Some (_, v) -> v
      | None -> VNull)
  | VString s, VInt i -> (
      let idx = Int64.to_int i in
      if idx >= 0 && idx < String.length s then
        VString (String.make 1 s.[idx])
      else raise (Eval_error "String index out of bounds"))
  | _ ->
      raise
        (Type_error
           (Printf.sprintf "Cannot index %s with %s" (type_name obj)
              (type_name index)))

(** Function registry - use centralized stdlib registry *)
let lookup_function (name : string) : func option =
  Stdlib_registry.lookup_function name

(** Main evaluation function *)
let rec eval (ctx : context) (expr : expr) : value =
  match expr with
  (* Literals *)
  | Literal v -> v

  (* Identifiers *)
  | Ident name -> lookup_ident ctx name

  (* Binary operations *)
  | Binary { op; left; right } ->
      let left_val = eval ctx left in
      (* Short-circuit evaluation for logical operators *)
      (match op with
      | OpAnd when not (is_truthy left_val) -> VBool false
      | OpOr when is_truthy left_val -> VBool true
      | _ ->
          let right_val = eval ctx right in
          eval_binary_op op left_val right_val)

  (* Unary operations *)
  | Unary { op; operand } ->
      let operand_val = eval ctx operand in
      eval_unary_op op operand_val

  (* Member access *)
  | Member { object_; field } ->
      let obj_val = eval ctx object_ in
      get_member obj_val field

  (* Index access *)
  | Index { object_; index } ->
      let obj_val = eval ctx object_ in
      let index_val = eval ctx index in
      get_index obj_val index_val

  (* Function call *)
  | Call { function_; args } ->
      let arg_values = List.map (eval ctx) args in
      call_function function_ arg_values ctx eval_lambda

  (* Method call: obj.method(args) → method(obj, ...args) *)
  | MethodCall { object_; method_; args } ->
      let obj_val = eval ctx object_ in
      let arg_values = List.map (eval ctx) args in
      (* Call method as function with object as first argument *)
      call_function method_ (obj_val :: arg_values) ctx eval_lambda

  (* List literal *)
  | ListLit items -> VList (List.map (eval ctx) items)

  (* Map literal *)
  | MapLit entries ->
      let eval_entry (k, v) = (eval ctx k, eval ctx v) in
      VMap (List.map eval_entry entries)

  (* Ternary conditional *)
  | Ternary { condition; if_true; if_false } ->
      let cond_val = eval ctx condition in
      if is_truthy cond_val then eval ctx if_true else eval ctx if_false

  (* Lambda - stored as value for use in higher-order functions *)
  | Lambda _ as lambda_expr ->
      (* Lambdas evaluate to VLambda, preserving the expression for later use *)
      VLambda lambda_expr

(** Evaluate lambda with parameter binding *)
and eval_lambda (lambda : expr) (param_value : value) (ctx : context) : value =
  match lambda with
  | Lambda { param; body } ->
      (* Add parameter to context *)
      let ctx' = (param, param_value) :: ctx in
      eval ctx' body
  | _ -> raise (Type_error "Expected lambda expression")

(** Call stdlib function - part of mutual recursion with eval *)
and call_function (name : string) (args : value list) (ctx : context) (eval_lambda_fn : lambda_eval) : value =
  match lookup_function name with
  | Some func -> func.impl args ctx eval_lambda_fn
  | None -> raise (Unknown_function name)

(** Fast variable lookup using hash table - O(1) instead of O(n) *)
let lookup_ident_fast (ctx : fast_context) (name : string) : value =
  match Hashtbl.find_opt ctx name with
  | Some v -> v
  | None -> raise (Unknown_identifier name)

(** Fast evaluator using hash table context for O(1) variable lookups
    PERFORMANCE: Critical for canvas rendering. Use this instead of eval when
    evaluating 10,000+ expressions per frame with the same context variables.
*)
let rec eval_fast (ctx : fast_context) (expr : expr) : value =
  match expr with
  (* Literals *)
  | Literal v -> v

  (* Identifiers - O(1) hash table lookup instead of O(n) list *)
  | Ident name -> lookup_ident_fast ctx name

  (* Binary operations *)
  | Binary { op; left; right } ->
      let left_val = eval_fast ctx left in
      (* Short-circuit evaluation for logical operators *)
      (match op with
      | OpAnd when not (is_truthy left_val) -> VBool false
      | OpOr when is_truthy left_val -> VBool true
      | _ ->
          let right_val = eval_fast ctx right in
          eval_binary_op op left_val right_val)

  (* Unary operations *)
  | Unary { op; operand } ->
      let operand_val = eval_fast ctx operand in
      eval_unary_op op operand_val

  (* Member access *)
  | Member { object_; field } ->
      let obj_val = eval_fast ctx object_ in
      get_member obj_val field

  (* Index access *)
  | Index { object_; index } ->
      let obj_val = eval_fast ctx object_ in
      let index_val = eval_fast ctx index in
      get_index obj_val index_val

  (* Function call - uses hash table lookup for function *)
  | Call { function_; args } ->
      let arg_values = List.map (eval_fast ctx) args in
      call_function_fast function_ arg_values ctx eval_lambda_fast

  (* Method call: obj.method(args) → method(obj, ...args) *)
  | MethodCall { object_; method_; args } ->
      let obj_val = eval_fast ctx object_ in
      let arg_values = List.map (eval_fast ctx) args in
      (* Call method as function with object as first argument *)
      call_function_fast method_ (obj_val :: arg_values) ctx eval_lambda_fast

  (* List literal *)
  | ListLit items -> VList (List.map (eval_fast ctx) items)

  (* Map literal *)
  | MapLit entries ->
      let eval_entry (k, v) = (eval_fast ctx k, eval_fast ctx v) in
      VMap (List.map eval_entry entries)

  (* Ternary conditional *)
  | Ternary { condition; if_true; if_false } ->
      let cond_val = eval_fast ctx condition in
      if is_truthy cond_val then eval_fast ctx if_true else eval_fast ctx if_false

  (* Lambda - stored as value for use in higher-order functions *)
  | Lambda _ as lambda_expr ->
      VLambda lambda_expr

(** Fast lambda evaluation with hash table context *)
and eval_lambda_fast (lambda : expr) (param_value : value) (ctx : fast_context) : value =
  match lambda with
  | Lambda { param; body } ->
      (* Add parameter to hash table context *)
      Hashtbl.add ctx param param_value;
      let result = eval_fast ctx body in
      (* Remove parameter from context to restore original state *)
      Hashtbl.remove ctx param;
      result
  | _ -> raise (Type_error "Expected lambda expression")

(** Fast function call using hash table context *)
and call_function_fast (name : string) (args : value list) (ctx : fast_context) (eval_lambda_fn : expr -> value -> fast_context -> value) : value =
  match lookup_function name with
  | Some func ->
      (* Convert hash table context to list for stdlib functions *)
      let list_ctx = Hashtbl.fold (fun k v acc -> (k, v) :: acc) ctx [] in
      (* Call function with list context (stdlib functions expect this) *)
      func.impl args list_ctx (fun lambda param list_ctx ->
        (* Convert back to hash table for fast evaluation *)
        let fast_ctx = Hashtbl.create (List.length list_ctx) in
        List.iter (fun (k, v) -> Hashtbl.add fast_ctx k v) list_ctx;
        eval_lambda_fn lambda param fast_ctx
      )
  | None -> raise (Unknown_function name)

(** Evaluate with JSON context (for JavaScript interop) *)
let eval_with_json_context (expr : expr) (json_context : string) : string =
  try
    (* Parse JSON context *)
    let json = Yojson.Basic.from_string json_context in
    let ctx_value = json_to_value json in

    (* Extract key-value pairs from context *)
    let ctx =
      match ctx_value with
      | VMap entries ->
          List.map
            (fun (k, v) ->
              match k with
              | VString key -> (key, v)
              | _ -> raise (Type_error "Context keys must be strings"))
            entries
      | _ -> raise (Type_error "Context must be an object")
    in

    (* Evaluate *)
    let result = eval ctx expr in

    (* Convert result to JSON *)
    Yojson.Basic.to_string (value_to_json result)
  with
  | Type_error msg | Eval_error msg | Unknown_identifier msg | Unknown_function msg ->
      (* Return error as JSON *)
      Yojson.Basic.to_string (`Assoc [ ("error", `String msg) ])
  | exn ->
      Yojson.Basic.to_string
        (`Assoc [ ("error", `String (Printexc.to_string exn)) ])

(* ============================================================================ *)
(* CEL Standard Library *)
