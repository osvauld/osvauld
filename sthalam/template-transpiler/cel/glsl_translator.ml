(** GLSL Translator - Convert CEL AST to GLSL Fragment Shader Code

    This module translates pure mathematical CEL expressions into GLSL
    fragment shader code for GPU-accelerated canvas pattern rendering.

    Supported:
    - Math functions: sin, cos, tan, sqrt, pow, abs, floor, ceil, round, sign, min, max
    - Trig functions: asin, acos, atan, atan2
    - Geometry functions: distance, lerp, clamp, map_range
    - Variables: x, y, time, gridSize, mouseX, mouseY (become uniforms)
    - Binary operators: +, -, *, /, <, <=, >, >=, ==, !=, &&, ||
    - Unary operators: -, !
    - Literals: numbers, booleans
    - Ternary: condition ? if_true : if_false

    Not Supported (will error):
    - Strings, maps, lists, lambdas
    - Custom functions beyond stdlib math/geometry
    - Member access, index access
*)

open Cel_types

(** Type for uniform variables extracted from expression *)
type uniform = {
  name: string;        (* Uniform name in GLSL: u_time, u_mouseX *)
  cel_var: string;     (* Variable name in CEL context: time, mouseX *)
  glsl_type: string;   (* GLSL type: "float", "vec2", "int" *)
}

(** Translation result *)
type translation_result = {
  success: bool;
  glsl_expr: string;        (* GLSL expression code *)
  uniforms: uniform list;   (* Variables that need uniforms *)
  error: string option;     (* Error message if failed *)
}

(** Reserved identifiers that become uniforms *)
let is_uniform_var = function
  | "x" | "y" -> false  (* These are built-in varyings, not uniforms *)
  | _ -> true  (* All other identifiers are treated as uniforms *)

(** Get GLSL type for uniform variable *)
let uniform_glsl_type = function
  | "time" | "gridSize" | "mouseX" | "mouseY" -> "float"
  | _ -> "float"  (* Default to float *)

(** Convert CEL variable to GLSL uniform name *)
let to_uniform_name var = "u_" ^ var

(** Translate binary operator *)
let translate_binary_op = function
  | OpAdd -> "+"
  | OpSub -> "-"
  | OpMul -> "*"
  | OpDiv -> "/"
  | OpMod -> "mod"  (* GLSL uses mod() function, we'll handle specially *)
  | OpLt -> "<"
  | OpLe -> "<="
  | OpGt -> ">"
  | OpGe -> ">="
  | OpEq -> "=="
  | OpNe -> "!="
  | OpAnd -> "&&"
  | OpOr -> "||"
  | OpIn -> raise (Type_error "GLSL: 'in' operator not supported")

(** Translate unary operator *)
let translate_unary_op = function
  | OpNot -> "!"
  | OpNeg -> "-"

(** Accumulator for collecting uniforms during translation *)
let uniforms_ref : uniform list ref = ref []

(** Add uniform to collection if not already present *)
let add_uniform var =
  if is_uniform_var var then
    let uniform_name = to_uniform_name var in
    if not (List.exists (fun u -> u.name = uniform_name) !uniforms_ref) then
      uniforms_ref := {
        name = uniform_name;
        cel_var = var;
        glsl_type = uniform_glsl_type var;
      } :: !uniforms_ref

(** Translate CEL expression to GLSL *)
let rec translate_expr (expr : expr) : string =
  match expr with
  | Literal v -> translate_literal v

  | Ident name ->
      if name = "x" || name = "y" then
        (* Built-in varyings from vertex shader *)
        name
      else if is_uniform_var name then (
        (* Add to uniforms and return uniform reference *)
        add_uniform name;
        to_uniform_name name
      ) else
        raise (Unknown_identifier ("GLSL: Unknown variable '" ^ name ^ "'"))

  | Binary { op = OpMod; left; right } ->
      (* GLSL mod is a function, not operator *)
      "mod(" ^ translate_expr left ^ ", " ^ translate_expr right ^ ")"

  | Binary { op; left; right } ->
      let left_glsl = translate_expr left in
      let right_glsl = translate_expr right in
      let op_glsl = translate_binary_op op in
      "(" ^ left_glsl ^ " " ^ op_glsl ^ " " ^ right_glsl ^ ")"

  | Unary { op; operand } ->
      let operand_glsl = translate_expr operand in
      let op_glsl = translate_unary_op op in
      "(" ^ op_glsl ^ operand_glsl ^ ")"

  | Ternary { condition; if_true; if_false } ->
      let cond_glsl = translate_expr condition in
      let true_glsl = translate_expr if_true in
      let false_glsl = translate_expr if_false in
      "(" ^ cond_glsl ^ " ? " ^ true_glsl ^ " : " ^ false_glsl ^ ")"

  | Call { function_; args } ->
      translate_function_call function_ args

  | Member _ ->
      raise (Type_error "GLSL: Member access not supported")

  | Index _ ->
      raise (Type_error "GLSL: Index access not supported")

  | MethodCall _ ->
      raise (Type_error "GLSL: Method calls not supported")

  | ListLit _ ->
      raise (Type_error "GLSL: List literals not supported")

  | MapLit _ ->
      raise (Type_error "GLSL: Map literals not supported")

  | Lambda _ ->
      raise (Type_error "GLSL: Lambdas not supported")

(** Translate literal value to GLSL *)
and translate_literal = function
  | VNull -> raise (Type_error "GLSL: null not supported")
  | VBool true -> "true"
  | VBool false -> "false"
  | VInt i -> Int64.to_string i ^ ".0"  (* GLSL requires .0 for float literals *)
  | VUint u -> Int64.to_string u ^ ".0"
  | VFloat f ->
      let s = string_of_float f in
      (* Ensure float has decimal point *)
      if String.contains s '.' || String.contains s 'e' || String.contains s 'E' then
        s
      else
        s ^ ".0"
  | VString _ -> raise (Type_error "GLSL: strings not supported")
  | VBytes _ -> raise (Type_error "GLSL: bytes not supported")
  | VList _ -> raise (Type_error "GLSL: lists not supported")
  | VMap _ -> raise (Type_error "GLSL: maps not supported")
  | VTimestamp _ -> raise (Type_error "GLSL: timestamps not supported")
  | VDuration _ -> raise (Type_error "GLSL: durations not supported")
  | VLambda _ -> raise (Type_error "GLSL: lambdas not supported")

(** Translate function call to GLSL *)
and translate_function_call name args =
  let argc = List.length args in
  let translate_args () = List.map translate_expr args in

  match name with
  (* Direct GLSL equivalents *)
  | "sin" | "cos" | "tan" | "asin" | "acos" | "atan"
  | "sqrt" | "abs" | "floor" | "ceil" | "round"
  | "exp" | "log" | "sign" when argc = 1 ->
      name ^ "(" ^ String.concat ", " (translate_args ()) ^ ")"

  | "min" | "max" | "pow" when argc = 2 ->
      name ^ "(" ^ String.concat ", " (translate_args ()) ^ ")"

  | "atan2" when argc = 2 ->
      (* GLSL atan(y, x) vs CEL atan2(y, x) - same order *)
      "atan(" ^ String.concat ", " (translate_args ()) ^ ")"

  (* Geometry functions *)
  | "distance" when argc = 4 ->
      let args_glsl = translate_args () in
      let x1 = List.nth args_glsl 0 in
      let y1 = List.nth args_glsl 1 in
      let x2 = List.nth args_glsl 2 in
      let y2 = List.nth args_glsl 3 in
      "distance(vec2(" ^ x1 ^ ", " ^ y1 ^ "), vec2(" ^ x2 ^ ", " ^ y2 ^ "))"

  | "lerp" when argc = 3 ->
      let args_glsl = translate_args () in
      let a = List.nth args_glsl 0 in
      let b = List.nth args_glsl 1 in
      let t = List.nth args_glsl 2 in
      "mix(" ^ a ^ ", " ^ b ^ ", " ^ t ^ ")"

  | "clamp" when argc = 3 ->
      "clamp(" ^ String.concat ", " (translate_args ()) ^ ")"

  | "map_range" when argc = 5 ->
      (* map_range(value, in_min, in_max, out_min, out_max) *)
      let args_glsl = translate_args () in
      let value = List.nth args_glsl 0 in
      let in_min = List.nth args_glsl 1 in
      let in_max = List.nth args_glsl 2 in
      let out_min = List.nth args_glsl 3 in
      let out_max = List.nth args_glsl 4 in
      (* Formula: out_min + (value - in_min) * (out_max - out_min) / (in_max - in_min) *)
      "(" ^ out_min ^ " + (" ^ value ^ " - " ^ in_min ^ ") * (" ^ out_max ^ " - " ^ out_min ^ ") / (" ^ in_max ^ " - " ^ in_min ^ "))"

  (* Math constants *)
  | "pi" when argc = 0 -> "3.14159265359"
  | "e" when argc = 0 -> "2.71828182846"

  (* Random - GLSL doesn't have built-in random, use fract(sin()) trick *)
  | "random" when argc = 0 ->
      (* Pseudo-random using time uniform *)
      add_uniform "time";
      "fract(sin(u_time * 12.9898) * 43758.5453)"

  | "random" when argc = 1 ->
      let seed = List.hd (translate_args ()) in
      "fract(sin(" ^ seed ^ " * 12.9898) * 43758.5453)"

  (* Unsupported functions *)
  | _ ->
      raise (Unknown_function ("GLSL: Function '" ^ name ^ "' with " ^ string_of_int argc ^ " arguments not supported in GPU mode"))

(** Main translation entry point *)
let translate (expr : expr) : translation_result =
  try
    (* Reset uniforms accumulator *)
    uniforms_ref := [];

    (* Translate expression *)
    let glsl_code = translate_expr expr in

    (* Return success with collected uniforms *)
    {
      success = true;
      glsl_expr = glsl_code;
      uniforms = List.rev !uniforms_ref;
      error = None;
    }
  with
  | Type_error msg | Eval_error msg | Unknown_identifier msg | Unknown_function msg ->
      {
        success = false;
        glsl_expr = "";
        uniforms = [];
        error = Some msg;
      }
  | exn ->
      {
        success = false;
        glsl_expr = "";
        uniforms = [];
        error = Some ("GLSL translation error: " ^ Printexc.to_string exn);
      }

(** Generate complete fragment shader code *)
let generate_fragment_shader (glsl_expr : string) (uniforms : uniform list) (grid_size : int) : string =
  let uniform_declarations =
    List.map (fun u -> "uniform " ^ u.glsl_type ^ " " ^ u.name ^ ";") uniforms
    |> String.concat "\n"
  in

  let shader = Printf.sprintf {|#version 300 es
precision highp float;

// Inputs from vertex shader
in vec2 v_texCoord;

// Outputs
out vec4 outColor;

// Uniforms
%s

void main() {
  // Convert texture coordinates (0-1) to grid coordinates (0-gridSize)
  vec2 pos = v_texCoord * %d.0;
  float x = pos.x;
  float y = pos.y;

  // Evaluate pattern expression
  float value = %s;

  // Convert value (-1 to 1) to grayscale color (0 to 1)
  float brightness = (value + 1.0) * 0.5;

  outColor = vec4(vec3(brightness), 1.0);
}
|} uniform_declarations grid_size glsl_expr in

  shader
