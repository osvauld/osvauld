(** Geometry Functions for CEL Evaluator

    Geometric utility functions for canvas patterns and interactive features.
    Includes distance calculations, interpolation, clamping, and coordinate transformations.

    All functions accept both VInt and VFloat inputs, converting VInt to float as needed.
*)

open Cel_types

(** Helper: Convert value to float for geometric operations *)
let to_float = function
  | VFloat f -> f
  | VInt i -> Int64.to_float i
  | _ -> raise (Type_error "Expected number (int or float)")

(** Geometry functions module *)
module Geometry_funcs = struct
  let distance args _ctx _eval =
    match args with
    | [x1; y1; x2; y2] ->
        let x1_f = to_float x1 in
        let y1_f = to_float y1 in
        let x2_f = to_float x2 in
        let y2_f = to_float y2 in
        let dx = x2_f -. x1_f in
        let dy = y2_f -. y1_f in
        VFloat (Float.sqrt (dx *. dx +. dy *. dy))
    | _ -> raise (Type_error "distance(x1, y1, x2, y2)")

  let lerp args _ctx _eval =
    match args with
    | [a; b; t] ->
        let a_f = to_float a in
        let b_f = to_float b in
        let t_f = to_float t in
        VFloat (a_f +. (b_f -. a_f) *. t_f)
    | _ -> raise (Type_error "lerp(a, b, t)")

  let clamp args _ctx _eval =
    match args with
    | [value; min_v; max_v] ->
        let v = to_float value in
        let min_f = to_float min_v in
        let max_f = to_float max_v in
        VFloat (Float.max min_f (Float.min v max_f))
    | _ -> raise (Type_error "clamp(value, min, max)")

  let map_range args _ctx _eval =
    match args with
    | [value; in_min; in_max; out_min; out_max] ->
        let v = to_float value in
        let in_min_f = to_float in_min in
        let in_max_f = to_float in_max in
        let out_min_f = to_float out_min in
        let out_max_f = to_float out_max in
        let normalized = (v -. in_min_f) /. (in_max_f -. in_min_f) in
        let mapped = normalized *. (out_max_f -. out_min_f) +. out_min_f in
        VFloat mapped
    | _ -> raise (Type_error "map_range(value, in_min, in_max, out_min, out_max)")

  let normalize args _ctx _eval =
    match args with
    | [x; y] ->
        let x_f = to_float x in
        let y_f = to_float y in
        let magnitude = Float.sqrt (x_f *. x_f +. y_f *. y_f) in
        if magnitude = 0.0 then
          VMap [(VString "x", VFloat 0.0); (VString "y", VFloat 0.0)]
        else
          VMap [(VString "x", VFloat (x_f /. magnitude)); (VString "y", VFloat (y_f /. magnitude))]
    | _ -> raise (Type_error "normalize(x, y)")

  let angle args _ctx _eval =
    match args with
    | [x1; y1; x2; y2] ->
        let x1_f = to_float x1 in
        let y1_f = to_float y1 in
        let x2_f = to_float x2 in
        let y2_f = to_float y2 in
        VFloat (Float.atan2 (y2_f -. y1_f) (x2_f -. x1_f))
    | _ -> raise (Type_error "angle(x1, y1, x2, y2)")

  let degrees args _ctx _eval =
    match args with
    | [radians] ->
        let rad = to_float radians in
        VFloat (rad *. 180.0 /. Float.pi)
    | _ -> raise (Type_error "degrees(radians)")

  let radians args _ctx _eval =
    match args with
    | [deg] ->
        let degrees = to_float deg in
        VFloat (degrees *. Float.pi /. 180.0)
    | _ -> raise (Type_error "radians(degrees)")
end

(** Get all geometry functions for registry *)
let get_geometry_functions () : func list = [
  { name = "distance"; impl = Geometry_funcs.distance };
  { name = "lerp"; impl = Geometry_funcs.lerp };
  { name = "clamp"; impl = Geometry_funcs.clamp };
  { name = "map_range"; impl = Geometry_funcs.map_range };
  { name = "normalize"; impl = Geometry_funcs.normalize };
  { name = "angle"; impl = Geometry_funcs.angle };
  { name = "degrees"; impl = Geometry_funcs.degrees };
  { name = "radians"; impl = Geometry_funcs.radians };
]
