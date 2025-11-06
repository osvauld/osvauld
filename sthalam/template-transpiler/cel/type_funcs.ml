(** Type Conversion Functions for CEL Evaluator

    Standard library type conversion functions (int, double, string).
*)

open Cel_types

(** Type conversion functions module *)
module Type_funcs = struct
  let to_int args _ctx _eval =
    match args with
    | [VInt i] -> VInt i
    | [VFloat f] -> VInt (Int64.of_float f)
    | [VString s] -> (
        try VInt (Int64.of_string s)
        with Failure _ -> raise (Eval_error ("Cannot convert string to int: " ^ s)))
    | [VBool true] -> VInt 1L
    | [VBool false] -> VInt 0L
    | _ -> raise (Type_error "int(int|float|string|bool)")

  let to_double args _ctx _eval =
    match args with
    | [VInt i] -> VFloat (Int64.to_float i)
    | [VFloat f] -> VFloat f
    | [VString s] -> (
        try VFloat (float_of_string s)
        with Failure _ -> raise (Eval_error ("Cannot convert string to double: " ^ s)))
    | _ -> raise (Type_error "double(int|float|string)")

  let to_string args _ctx _eval =
    match args with
    | [v] -> VString (value_to_string v)
    | _ -> raise (Type_error "string(any)")
end

(** Get all type conversion functions for registry *)
let get_type_functions () : func list = [
  { name = "int"; impl = Type_funcs.to_int };
  { name = "double"; impl = Type_funcs.to_double };
  { name = "string"; impl = Type_funcs.to_string };
]
