(** Math Functions for CEL Evaluator

    Mathematical functions for canvas patterns and computational expressions.
    Includes trigonometry, power/root operations, rounding, and comparison functions.

    All functions accept both VInt and VFloat inputs, converting VInt to float as needed.
*)

open Cel_types

(** Helper: Convert value to float for math operations *)
let to_float = function
  | VFloat f -> f
  | VInt i -> Int64.to_float i
  | _ -> raise (Type_error "Expected number (int or float)")

(** Math functions module *)
module Math_funcs = struct
  (* Trigonometric functions *)

  let sin args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.sin (to_float v))
    | _ -> raise (Type_error "sin(number)")

  let cos args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.cos (to_float v))
    | _ -> raise (Type_error "cos(number)")

  let tan args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.tan (to_float v))
    | _ -> raise (Type_error "tan(number)")

  let asin args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.asin (to_float v))
    | _ -> raise (Type_error "asin(number)")

  let acos args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.acos (to_float v))
    | _ -> raise (Type_error "acos(number)")

  let atan args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.atan (to_float v))
    | _ -> raise (Type_error "atan(number)")

  let atan2 args _ctx _eval =
    match args with
    | [y; x] -> VFloat (Float.atan2 (to_float y) (to_float x))
    | _ -> raise (Type_error "atan2(y, x)")

  (* Power and root functions *)

  let sqrt args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.sqrt (to_float v))
    | _ -> raise (Type_error "sqrt(number)")

  let pow args _ctx _eval =
    match args with
    | [base; exp] -> VFloat (Float.pow (to_float base) (to_float exp))
    | _ -> raise (Type_error "pow(base, exponent)")

  let exp args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.exp (to_float v))
    | _ -> raise (Type_error "exp(number)")

  let log args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.log (to_float v))
    | _ -> raise (Type_error "log(number)")

  let log10 args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.log10 (to_float v))
    | _ -> raise (Type_error "log10(number)")

  (* Rounding functions *)

  let floor args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.floor (to_float v))
    | _ -> raise (Type_error "floor(number)")

  let ceil args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.ceil (to_float v))
    | _ -> raise (Type_error "ceil(number)")

  let round args _ctx _eval =
    match args with
    | [v] -> VFloat (Float.round (to_float v))
    | _ -> raise (Type_error "round(number)")

  let abs args _ctx _eval =
    match args with
    | [VFloat f] -> VFloat (Float.abs f)
    | [VInt i] -> VInt (Int64.abs i)
    | _ -> raise (Type_error "abs(number)")

  let sign args _ctx _eval =
    match args with
    | [v] ->
        let f = to_float v in
        if f > 0.0 then VInt 1L
        else if f < 0.0 then VInt (-1L)
        else VInt 0L
    | _ -> raise (Type_error "sign(number)")

  (* Comparison functions *)

  let min args _ctx _eval =
    match args with
    | [] -> raise (Type_error "min() requires at least one argument")
    | values ->
        let floats = List.map to_float values in
        VFloat (List.fold_left Float.min (List.hd floats) (List.tl floats))

  let max args _ctx _eval =
    match args with
    | [] -> raise (Type_error "max() requires at least one argument")
    | values ->
        let floats = List.map to_float values in
        VFloat (List.fold_left Float.max (List.hd floats) (List.tl floats))

  (* Random function *)

  let random args _ctx _eval =
    match args with
    | [] ->
        Random.self_init ();
        VFloat (Random.float 1.0)
    | _ -> raise (Type_error "random()")

  (* Mathematical constants *)

  let pi _args _ctx _eval =
    VFloat Float.pi

  let e _args _ctx _eval =
    VFloat 2.718281828459045  (* Float.e is not available, so we define it *)
end

(** Get all math functions for registry *)
let get_math_functions () : func list = [
  (* Trigonometric *)
  { name = "sin"; impl = Math_funcs.sin };
  { name = "cos"; impl = Math_funcs.cos };
  { name = "tan"; impl = Math_funcs.tan };
  { name = "asin"; impl = Math_funcs.asin };
  { name = "acos"; impl = Math_funcs.acos };
  { name = "atan"; impl = Math_funcs.atan };
  { name = "atan2"; impl = Math_funcs.atan2 };

  (* Power and root *)
  { name = "sqrt"; impl = Math_funcs.sqrt };
  { name = "pow"; impl = Math_funcs.pow };
  { name = "exp"; impl = Math_funcs.exp };
  { name = "log"; impl = Math_funcs.log };
  { name = "log10"; impl = Math_funcs.log10 };

  (* Rounding *)
  { name = "floor"; impl = Math_funcs.floor };
  { name = "ceil"; impl = Math_funcs.ceil };
  { name = "round"; impl = Math_funcs.round };
  { name = "abs"; impl = Math_funcs.abs };
  { name = "sign"; impl = Math_funcs.sign };

  (* Comparison *)
  { name = "min"; impl = Math_funcs.min };
  { name = "max"; impl = Math_funcs.max };

  (* Random *)
  { name = "random"; impl = Math_funcs.random };

  (* Constants *)
  { name = "pi"; impl = Math_funcs.pi };
  { name = "e"; impl = Math_funcs.e };
]
