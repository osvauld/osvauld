(** CEL (Common Expression Language) AST Types

    Based on CEL spec: https://github.com/google/cel-spec

    Core design:
    - Simple, minimal AST matching CEL grammar
    - Type-safe value representation
    - Clean separation from evaluation logic
*)

(** Value types that CEL expressions evaluate to *)
type value =
  | VNull
  | VBool of bool
  | VInt of int64          (* CEL int (signed 64-bit) *)
  | VUint of int64         (* CEL uint (unsigned 64-bit) *)
  | VFloat of float        (* CEL double (IEEE 754) *)
  | VString of string
  | VBytes of bytes
  | VList of value list
  | VMap of (value * value) list
  | VTimestamp of float    (* Unix timestamp *)
  | VDuration of float     (* Duration in seconds *)
  | VLambda of expr        (* Unevaluated lambda for higher-order functions *)

(** Binary operators (matching CEL precedence) *)
and binary_op =
  (* Relational *)
  | OpLt           (* < *)
  | OpLe           (* <= *)
  | OpGt           (* > *)
  | OpGe           (* >= *)
  | OpEq           (* == *)
  | OpNe           (* != *)
  | OpIn           (* in *)
  (* Arithmetic *)
  | OpAdd          (* + *)
  | OpSub          (* - *)
  | OpMul          (* * *)
  | OpDiv          (* / *)
  | OpMod          (* % *)
  (* Logical *)
  | OpAnd          (* && *)
  | OpOr           (* || *)

(** Unary operators *)
and unary_op =
  | OpNot          (* ! *)
  | OpNeg          (* - (unary) *)

(** CEL Expression AST *)
and expr =
  (* Literals *)
  | Literal of value

  (* Identifiers *)
  | Ident of string

  (* Binary operations *)
  | Binary of {
      op: binary_op;
      left: expr;
      right: expr;
    }

  (* Unary operations *)
  | Unary of {
      op: unary_op;
      operand: expr;
    }

  (* Member access: obj.field *)
  | Member of {
      object_: expr;
      field: string;
    }

  (* Index access: obj[index] *)
  | Index of {
      object_: expr;
      index: expr;
    }

  (* Function call: func(args) or obj.method(args) *)
  | Call of {
      function_: string;
      args: expr list;
    }

  (* Method call: obj.method(args) *)
  | MethodCall of {
      object_: expr;
      method_: string;
      args: expr list;
    }

  (* List literal: [e1, e2, ..., eN] *)
  | ListLit of expr list

  (* Map literal: {k1: v1, k2: v2, ...} *)
  | MapLit of (expr * expr) list

  (* Ternary conditional: cond ? true_expr : false_expr *)
  | Ternary of {
      condition: expr;
      if_true: expr;
      if_false: expr;
    }

  (* Lambda for comprehensions: x => x * 2 *)
  | Lambda of {
      param: string;
      body: expr;
    }

(** Convert value to string for display *)
let rec value_to_string = function
  | VNull -> "null"
  | VBool b -> string_of_bool b
  | VInt i -> Int64.to_string i
  | VUint u -> Int64.to_string u ^ "u"
  | VFloat f -> string_of_float f
  | VString s -> "\"" ^ String.escaped s ^ "\""
  | VBytes b -> "b\"" ^ (Bytes.to_string b |> String.escaped) ^ "\""
  | VList items ->
      let items_str = List.map value_to_string items |> String.concat ", " in
      "[" ^ items_str ^ "]"
  | VMap entries ->
      let entry_str (k, v) =
        value_to_string k ^ ": " ^ value_to_string v
      in
      let entries_str = List.map entry_str entries |> String.concat ", " in
      "{" ^ entries_str ^ "}"
  | VTimestamp t -> "timestamp(" ^ string_of_float t ^ ")"
  | VDuration d -> "duration(" ^ string_of_float d ^ "s)"
  | VLambda _ -> "<lambda>"

(** Convert value to JSON (for JavaScript interop) *)
let rec value_to_json = function
  | VNull -> `Null
  | VBool b -> `Bool b
  | VInt i -> `Int (Int64.to_int i)  (* May lose precision *)
  | VUint u -> `Int (Int64.to_int u)
  | VFloat f -> `Float f
  | VString s -> `String s
  | VBytes b -> `String (Bytes.to_string b)
  | VList items -> `List (List.map value_to_json items)
  | VMap entries ->
      let to_assoc (k, v) =
        (value_to_string k, value_to_json v)
      in
      `Assoc (List.map to_assoc entries)
  | VTimestamp t -> `Float t
  | VDuration d -> `Float d
  | VLambda _ -> `String "<lambda>"

(** Convert JSON to value *)
let rec json_to_value = function
  | `Null -> VNull
  | `Bool b -> VBool b
  | `Int i -> VInt (Int64.of_int i)
  | `Float f -> VFloat f
  | `String s -> VString s
  | `List items -> VList (List.map json_to_value items)
  | `Assoc entries ->
      let to_map (k, v) = (VString k, json_to_value v) in
      VMap (List.map to_map entries)
  | `Intlit s -> (
      try VInt (Int64.of_string s)
      with _ -> VFloat (float_of_string s))

(** Truthy check (CEL semantics) *)
let is_truthy = function
  | VNull -> false
  | VBool b -> b
  | VInt i -> i <> 0L
  | VUint u -> u <> 0L
  | VFloat f -> f <> 0.0 && not (Float.is_nan f)
  | VString s -> s <> ""
  | VBytes b -> Bytes.length b > 0
  | VList lst -> lst <> []
  | VMap m -> m <> []
  | VTimestamp _ -> true
  | VDuration _ -> true
  | VLambda _ -> true

(** Convert to boolean *)
let to_bool v = VBool (is_truthy v)

(** Compare values (for relational operators) *)
let compare_values v1 v2 =
  match (v1, v2) with
  | VNull, VNull -> 0
  | VBool b1, VBool b2 -> compare b1 b2
  | VInt i1, VInt i2 -> Int64.compare i1 i2
  | VUint u1, VUint u2 -> Int64.compare u1 u2
  | VFloat f1, VFloat f2 -> Float.compare f1 f2
  | VString s1, VString s2 -> String.compare s1 s2
  | VBytes b1, VBytes b2 -> Bytes.compare b1 b2
  | VInt i, VFloat f | VFloat f, VInt i ->
      Float.compare (Int64.to_float i) f
  | VUint u, VFloat f | VFloat f, VUint u ->
      Float.compare (Int64.to_float u) f
  | VTimestamp t1, VTimestamp t2 -> Float.compare t1 t2
  | VDuration d1, VDuration d2 -> Float.compare d1 d2
  | _ ->
      (* Type mismatch - convert to string comparison as fallback *)
      String.compare (value_to_string v1) (value_to_string v2)

(** Check equality (== operator) *)
let values_equal v1 v2 =
  compare_values v1 v2 = 0

(** Type names for error messages *)
let type_name = function
  | VNull -> "null"
  | VBool _ -> "bool"
  | VInt _ -> "int"
  | VUint _ -> "uint"
  | VFloat _ -> "double"
  | VString _ -> "string"
  | VBytes _ -> "bytes"
  | VList _ -> "list"
  | VMap _ -> "map"
  | VTimestamp _ -> "timestamp"
  | VDuration _ -> "duration"
  | VLambda _ -> "lambda"

(** Exception types *)
exception Type_error of string
exception Eval_error of string
exception Unknown_identifier of string
exception Unknown_function of string

(** Evaluation context - maps identifiers to values *)
type context = (string * value) list

(** Fast evaluation context using hash table for O(1) lookups
    PERFORMANCE: Critical for canvas rendering with 40,000+ variable lookups per frame.
    Use this for evaluate_grid instead of list-based context.
*)
type fast_context = (string, value) Hashtbl.t

(** Type for evaluating lambdas - passed to stdlib functions *)
type lambda_eval = expr -> value -> context -> value

(** Standard library function type *)
type func = {
  name: string;
  impl: value list -> context -> lambda_eval -> value;
}
