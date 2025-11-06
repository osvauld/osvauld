(** String Functions for CEL Evaluator

    Standard library string manipulation functions.
*)

open Cel_types

(** Helper: Check if string contains substring *)
let string_contains (str : string) (sub : string) : bool =
  try
    let _ = Str.search_forward (Str.regexp_string sub) str 0 in
    true
  with Not_found -> false

(** String functions module *)
module String_funcs = struct
  let contains args _ctx _eval =
    match args with
    | [VString s; VString sub] ->
        VBool (string_contains s sub)
    | _ -> raise (Type_error "contains(string, string)")

  let starts_with args _ctx _eval =
    match args with
    | [VString s; VString prefix] ->
        VBool (String.starts_with ~prefix s)
    | _ -> raise (Type_error "startsWith(string, string)")

  let ends_with args _ctx _eval =
    match args with
    | [VString s; VString suffix] ->
        VBool (String.ends_with ~suffix s)
    | _ -> raise (Type_error "endsWith(string, string)")

  let trim args _ctx _eval =
    match args with
    | [VString s] -> VString (String.trim s)
    | _ -> raise (Type_error "trim(string)")

  let to_lower_case args _ctx _eval =
    match args with
    | [VString s] -> VString (String.lowercase_ascii s)
    | _ -> raise (Type_error "toLowerCase(string)")

  let to_upper_case args _ctx _eval =
    match args with
    | [VString s] -> VString (String.uppercase_ascii s)
    | _ -> raise (Type_error "toUpperCase(string)")

  let split args _ctx _eval =
    match args with
    | [VString s; VString delim] ->
        if String.length delim = 0 then
          raise (Eval_error "split delimiter cannot be empty")
        else
          let parts = String.split_on_char delim.[0] s in
          VList (List.map (fun p -> VString p) parts)
    | _ -> raise (Type_error "split(string, string)")

  let replace args _ctx _eval =
    match args with
    | [VString s; VString old_str; VString new_str] ->
        let regex = Str.regexp_string old_str in
        VString (Str.global_replace regex new_str s)
    | _ -> raise (Type_error "replace(string, string, string)")

  let substring args _ctx _eval =
    match args with
    | [VString s; VInt start; VInt length] ->
        let start_idx = Int64.to_int start in
        let len = Int64.to_int length in
        if start_idx < 0 || start_idx >= String.length s then
          raise (Eval_error "substring start index out of bounds")
        else if len < 0 then
          raise (Eval_error "substring length must be non-negative")
        else
          let actual_len = min len (String.length s - start_idx) in
          VString (String.sub s start_idx actual_len)
    | _ -> raise (Type_error "substring(string, int, int)")
end

(** Get all string functions for registry *)
let get_string_functions () : func list = [
  { name = "contains"; impl = String_funcs.contains };
  { name = "startsWith"; impl = String_funcs.starts_with };
  { name = "endsWith"; impl = String_funcs.ends_with };
  { name = "trim"; impl = String_funcs.trim };
  { name = "toLowerCase"; impl = String_funcs.to_lower_case };
  { name = "toUpperCase"; impl = String_funcs.to_upper_case };
  { name = "split"; impl = String_funcs.split };
  { name = "replace"; impl = String_funcs.replace };
  { name = "substring"; impl = String_funcs.substring };
]
