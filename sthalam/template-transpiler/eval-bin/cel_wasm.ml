(** CEL WASM Bindings - Direct JavaScript Object Access

    This module provides JavaScript bindings for the CEL evaluator.
    Uses direct JS object access for maximum performance and lazy evaluation.
*)

open Js_of_ocaml

module Typed_array = Js_of_ocaml.Typed_array

(* Import CEL modules *)
open Cel.Cel_types
open Cel.Cel_eval
module Cel_parser = Cel.Cel_parser
module Cel_lexer = Cel.Cel_lexer
module Glsl_translator = Cel.Glsl_translator

(** Convert JavaScript value to CEL value *)
let rec js_to_value (js_val : Js.Unsafe.any) : value =
  let js_type = Js.to_string (Js.typeof js_val) in

  match js_type with
  | "undefined" -> VNull
  | "boolean" -> VBool (Js.to_bool (Js.Unsafe.coerce js_val))
  | "number" ->
      let num = float_of_js (Js.Unsafe.coerce js_val) in
      if Float.is_integer num then
        VInt (Int64.of_float num)
      else
        VFloat num
  | "string" -> VString (Js.to_string (Js.Unsafe.coerce js_val))
  | "object" ->
      if Js.Unsafe.equals js_val Js.null then
        VNull
      else if Js.to_bool (Js.Unsafe.fun_call
                           (Js.Unsafe.js_expr "Array.isArray")
                           [| js_val |]) then
        (* JavaScript array → CEL list *)
        let arr : Js.Unsafe.any Js.js_array Js.t = Js.Unsafe.coerce js_val in
        let length = arr##.length in
        let items = ref [] in
        for i = length - 1 downto 0 do
          let elem = Js.array_get arr i in
          match Js.Optdef.to_option elem with
          | Some v -> items := js_to_value v :: !items
          | None -> ()
        done;
        VList !items
      else
        (* JavaScript object → CEL map *)
        let keys = Js.object_keys js_val in
        let length = keys##.length in
        let pairs = ref [] in
        for i = 0 to length - 1 do
          match Js.Optdef.to_option (Js.array_get keys i) with
          | Some key_js ->
              let key_str = Js.to_string key_js in
              let value_js = Js.Unsafe.get js_val key_str in
              let cel_key = VString key_str in
              let cel_value = js_to_value value_js in
              pairs := (cel_key, cel_value) :: !pairs
          | None -> ()
        done;
        VMap (List.rev !pairs)
  | _ -> VNull

and float_of_js (js_val : Js.Unsafe.any) : float =
  Js.float_of_number (Js.Unsafe.coerce js_val)

(** Convert CEL value back to JavaScript *)
let rec value_to_js (v : value) : Js.Unsafe.any =
  match v with
  | VNull -> Js.Unsafe.inject Js.null
  | VBool b -> Js.Unsafe.inject (Js.bool b)
  | VInt i -> Js.Unsafe.inject (Js.number_of_float (Int64.to_float i))
  | VUint u -> Js.Unsafe.inject (Js.number_of_float (Int64.to_float u))
  | VFloat f -> Js.Unsafe.inject (Js.number_of_float f)
  | VString s -> Js.Unsafe.inject (Js.string s)
  | VBytes b -> Js.Unsafe.inject (Js.string (Bytes.to_string b))
  | VList items ->
      let arr = new%js Js.array_empty in
      List.iter (fun item ->
        let _ = arr##push (value_to_js item) in
        ()
      ) items;
      Js.Unsafe.inject arr
  | VMap pairs ->
      let obj = Js.Unsafe.obj [||] in
      List.iter (fun (k, v) ->
        match k with
        | VString key ->
            Js.Unsafe.set obj (Js.string key) (value_to_js v)
        | _ -> ()  (* Non-string keys ignored *)
      ) pairs;
      Js.Unsafe.inject obj
  | VTimestamp t -> Js.Unsafe.inject (Js.number_of_float t)
  | VDuration d -> Js.Unsafe.inject (Js.number_of_float d)
  | VLambda _ -> Js.Unsafe.inject (Js.string "<lambda>")

(** Create context from JavaScript object (lazy) *)
let context_from_js_object (js_obj : Js.Unsafe.any) : context =
  let keys = Js.object_keys js_obj in
  let length = keys##.length in
  let ctx = ref [] in
  for i = 0 to length - 1 do
    match Js.Optdef.to_option (Js.array_get keys i) with
    | Some key_js ->
        let key_str = Js.to_string key_js in
        let value_js = Js.Unsafe.get js_obj key_str in
        let cel_value = js_to_value value_js in
        ctx := (key_str, cel_value) :: !ctx
    | None -> ()
  done;
  List.rev !ctx

(** Evaluate CEL expression with JavaScript object context
    Supports pure expression syntax: ${ expr } or legacy expr
    @param expr_str CEL expression string (may be wrapped with ${ })
    @param context_obj JavaScript object (Loro map or regular JS object)
    @return JavaScript object { success: bool, result?: any, error?: string }
*)
let evaluate_expr (expr_js : Js.js_string Js.t) (context_obj : Js.Unsafe.any) =
  let expr_str = Js.to_string expr_js in

  (* Log expression being evaluated *)
  let () = Js.Unsafe.fun_call (Js.Unsafe.js_expr "console.log")
    [| Js.Unsafe.inject (Js.string ("[CEL] Evaluating: " ^ expr_str)) |] in

  try
    (* Strip ${ } markers if present (pure expression syntax) *)
    let clean_expr =
      let trimmed = String.trim expr_str in
      if String.length trimmed > 3 &&
         String.sub trimmed 0 2 = "${" &&
         String.get trimmed (String.length trimmed - 1) = '}' then
        (* Strip ${ } markers *)
        String.trim (String.sub trimmed 2 (String.length trimmed - 3))
      else
        trimmed
    in

    (* Parse expression *)
    let lexbuf = Lexing.from_string clean_expr in
    let ast = Cel_parser.main Cel_lexer.token lexbuf in

    (* Create context from JS object (lazy conversion) *)
    let context = context_from_js_object context_obj in

    (* Evaluate *)
    let result = eval context ast in

    (* Convert result back to JavaScript *)
    let js_result = value_to_js result in

    object%js
      val success = Js.bool true
      val result = js_result
      val error = Js.Unsafe.inject Js.null
    end
  with
  | Cel.Cel_types.Type_error msg ->
      object%js
        val success = Js.bool false
        val result = Js.Unsafe.inject Js.null
        val error = Js.Unsafe.inject (Js.string ("Type error: " ^ msg))
      end
  | Cel.Cel_types.Eval_error msg ->
      object%js
        val success = Js.bool false
        val result = Js.Unsafe.inject Js.null
        val error = Js.Unsafe.inject (Js.string ("Evaluation error: " ^ msg))
      end
  | Cel.Cel_types.Unknown_identifier msg ->
      object%js
        val success = Js.bool false
        val result = Js.Unsafe.inject Js.null
        val error = Js.Unsafe.inject (Js.string ("Unknown identifier: " ^ msg))
      end
  | Cel.Cel_types.Unknown_function msg ->
      object%js
        val success = Js.bool false
        val result = Js.Unsafe.inject Js.null
        val error = Js.Unsafe.inject (Js.string ("Unknown function: " ^ msg))
      end
  | exn ->
      let error_msg = Printexc.to_string exn in
      let () = Js.Unsafe.fun_call (Js.Unsafe.js_expr "console.error")
        [| Js.Unsafe.inject (Js.string ("[CEL] PARSE ERROR for: " ^ expr_str)) |] in
      let () = Js.Unsafe.fun_call (Js.Unsafe.js_expr "console.error")
        [| Js.Unsafe.inject (Js.string ("[CEL] Error: " ^ error_msg)) |] in
      object%js
        val success = Js.bool false
        val result = Js.Unsafe.inject Js.null
        val error = Js.Unsafe.inject (Js.string error_msg)
      end

(** Interpolate CEL expressions in template string
    Replaces {{ expr }} with evaluated results
    @param template Template string with {{ }} markers
    @param context_obj JavaScript object context
    @return Interpolated string
*)
let interpolate (template_js : Js.js_string Js.t) (context_obj : Js.Unsafe.any) =
  let template = Js.to_string template_js in

  try
    let context = context_from_js_object context_obj in

    (* Find and replace all {{ expr }} patterns *)
    let expr_regex = Str.regexp "{{\\([^}]+\\)}}" in
    let replace_expr matched =
      let expr_str = Str.matched_group 1 matched in
      let expr_str = String.trim expr_str in
      try
        let lexbuf = Lexing.from_string expr_str in
        let ast = Cel_parser.main Cel_lexer.token lexbuf in
        let result = eval context ast in
        (* Convert to string for interpolation - use raw value, not quoted representation *)
        match result with
        | VString s -> s  (* Raw string value *)
        | _ -> value_to_string result  (* For non-strings, use display format *)
      with
      | _ -> matched  (* Keep original on error *)
    in
    let result = Str.global_substitute expr_regex replace_expr template in
    Js.string result
  with
  | exn -> Js.string ("Error: " ^ Printexc.to_string exn)

(** Evaluate expression for grid (canvas pattern rendering)
    Highly optimized for 10,000+ evaluations per frame
    @param expr_str CEL expression string
    @param grid_size Grid size (100 means 100x100 = 10,000 cells)
    @param time Current time for animation
    @return Uint8Array with brightness values (0-255) for each cell
*)
let evaluate_grid
    (expr_js : Js.js_string Js.t)
    (grid_size_js : int)
    (time_js : Js.number Js.t) =
  let expr_str = Js.to_string expr_js in

  try
    (* Strip ${ } markers if present (pure expression syntax) *)
    let clean_expr =
      let trimmed = String.trim expr_str in
      if String.length trimmed > 3 &&
         String.sub trimmed 0 2 = "${" &&
         String.get trimmed (String.length trimmed - 1) = '}' then
        (* Strip ${ } markers *)
        String.trim (String.sub trimmed 2 (String.length trimmed - 3))
      else
        trimmed
    in

    (* Parse expression ONCE (not 10,000 times!) *)
    let lexbuf = Lexing.from_string clean_expr in
    let ast = Cel_parser.main Cel_lexer.token lexbuf in

    let grid_size = grid_size_js in
    let time = Js.float_of_number time_js in

    (* Create Uint8Array for zero-copy GPU upload *)
    let total_cells = grid_size * grid_size in
    let results = new%js Typed_array.uint8Array total_cells in

    (* PERFORMANCE OPTIMIZATION: Create hash table context once, reuse for all 10,000 cells
       This eliminates 40,000 O(n) list lookups per frame, achieving O(1) hash lookups instead.
    *)
    let fast_ctx = Hashtbl.create 4 in
    Hashtbl.add fast_ctx "time" (VFloat time);
    Hashtbl.add fast_ctx "gridSize" (VInt (Int64.of_int grid_size));

    (* Evaluate for each cell - reuse hash table, just update x/y values *)
    let idx = ref 0 in
    for y = 0 to grid_size - 1 do
      (* Replace y value in hash table (O(1) operation) *)
      Hashtbl.replace fast_ctx "y" (VInt (Int64.of_int y));

      for x = 0 to grid_size - 1 do
        (* Replace x value in hash table (O(1) operation) *)
        Hashtbl.replace fast_ctx "x" (VInt (Int64.of_int x));

        (* Fast evaluation using hash table context - O(1) variable lookups *)
        let brightness = match eval_fast fast_ctx ast with
          | VFloat f -> f
          | VInt i -> Int64.to_float i
          | _ -> 0.0
        in

        (* Convert brightness (-1.0 to 1.0) to color byte (0-255) *)
        let color = int_of_float ((brightness +. 1.0) *. 127.5) in
        let color_byte = max 0 (min 255 color) in

        (* Set byte in Uint8Array *)
        Typed_array.set results !idx color_byte;
        incr idx
      done
    done;

    (* Return result object with Uint8Array *)
    object%js
      val success = Js.bool true
      val output = results
      val error = Js.Unsafe.inject Js.null
    end
  with
  | exn ->
      object%js
        val success = Js.bool false
        val output = new%js Typed_array.uint8Array 0
        val error = Js.Unsafe.inject (Js.string (Printexc.to_string exn))
      end

(** Compile CEL expression to GLSL shader code
    Translates mathematical CEL expressions to GPU fragment shaders
    @param expr_str CEL expression string
    @param grid_size Grid size for shader (default: 100)
    @return JavaScript object { success, glsl_expr, shader_code, uniforms[], error }
*)
let compile_to_glsl (expr_js : Js.js_string Js.t) (grid_size_js : int) =
  let expr_str = Js.to_string expr_js in

  try
    (* Strip ${ } markers if present (pure expression syntax) *)
    let clean_expr =
      let trimmed = String.trim expr_str in
      if String.length trimmed > 3 &&
         String.sub trimmed 0 2 = "${" &&
         String.get trimmed (String.length trimmed - 1) = '}' then
        (* Strip ${ } markers *)
        String.trim (String.sub trimmed 2 (String.length trimmed - 3))
      else
        trimmed
    in

    (* Parse expression *)
    let lexbuf = Lexing.from_string clean_expr in
    let ast = Cel_parser.main Cel_lexer.token lexbuf in

    (* Translate to GLSL *)
    let translation = Glsl_translator.translate ast in

    if translation.success then
      (* Generate complete fragment shader *)
      let shader_code = Glsl_translator.generate_fragment_shader
        translation.glsl_expr
        translation.uniforms
        grid_size_js
      in

      (* Convert uniforms to JavaScript array *)
      let uniforms_arr = new%js Js.array_empty in
      List.iter (fun (u : Glsl_translator.uniform) ->
        let uniform_obj = object%js
          val name = Js.string u.name
          val celVar = Js.string u.cel_var
          val glslType = Js.string u.glsl_type
        end in
        let _ = uniforms_arr##push uniform_obj in
        ()
      ) translation.uniforms;

      object%js
        val success = Js.bool true
        val glslExpr = Js.string translation.glsl_expr
        val shaderCode = Js.string shader_code
        val uniforms = uniforms_arr
        val error = Js.Unsafe.inject Js.null
      end
    else
      (* Translation failed *)
      let error_msg = match translation.error with
        | Some msg -> msg
        | None -> "Unknown GLSL translation error"
      in
      object%js
        val success = Js.bool false
        val glslExpr = Js.string ""
        val shaderCode = Js.string ""
        val uniforms = new%js Js.array_empty
        val error = Js.Unsafe.inject (Js.string error_msg)
      end
  with
  | exn ->
      let error_msg = Printexc.to_string exn in
      object%js
        val success = Js.bool false
        val glslExpr = Js.string ""
        val shaderCode = Js.string ""
        val uniforms = new%js Js.array_empty
        val error = Js.Unsafe.inject (Js.string error_msg)
      end

(** Get version *)
let version () = Js.string "2.0.0"

(** Generate unique asset ID
    Format: asset_{type}_{timestamp}_{random}
    @param asset_type "video", "image", "file", or "audio"
    @return Unique asset ID string
*)
let generate_asset_id (asset_type_js : Js.js_string Js.t) =
  let asset_type = Js.to_string asset_type_js in

  (* Validate asset type *)
  let valid_type = match asset_type with
    | "video" | "image" | "file" | "audio" -> asset_type
    | _ -> "file"  (* Default to file for unknown types *)
  in

  (* Get current timestamp in milliseconds *)
  let timestamp = Int64.to_string (Int64.of_float (Js.to_float
    (Js.Unsafe.fun_call (Js.Unsafe.js_expr "Date.now") [||]))) in

  (* Generate random string (9 chars, base36) *)
  let random_float = Js.to_float
    (Js.Unsafe.fun_call (Js.Unsafe.js_expr "Math.random") [||]) in
  let random_int = int_of_float (random_float *. 1000000000.0) in
  let random_str = Printf.sprintf "%x" random_int in
  let random_trimmed = if String.length random_str > 9 then
    String.sub random_str 0 9
  else
    random_str
  in

  (* Format: asset_{type}_{timestamp}_{random} *)
  let asset_id = Printf.sprintf "asset_%s_%s_%s" valid_type timestamp random_trimmed in
  Js.string asset_id

(** Get supported functions *)
let supported_functions () =
  let funcs = [|
    (* String functions (9) *)
    "contains"; "startsWith"; "endsWith"; "trim";
    "toLowerCase"; "toUpperCase"; "split"; "replace"; "substring";
    (* Collection functions (11) *)
    "size"; "filter"; "map"; "exists"; "all";
    "flatten"; "unique"; "slice"; "exists_one"; "find"; "join";
    (* Type conversions (3) *)
    "int"; "double"; "string";
    (* Time functions (5) *)
    "now"; "timestamp"; "generateId"; "duration"; "format";
    (* Math functions (22) *)
    "sin"; "cos"; "tan"; "asin"; "acos"; "atan"; "atan2";
    "sqrt"; "pow"; "exp"; "log"; "log10";
    "floor"; "ceil"; "round"; "abs"; "sign";
    "min"; "max"; "random"; "pi"; "e";
    (* Geometry functions (8) *)
    "distance"; "lerp"; "clamp"; "map_range";
    "normalize"; "angle"; "degrees"; "radians";
  |] in
  let arr = new%js Js.array_empty in
  Array.iter (fun name ->
    let _ = arr##push (Js.string name) in
    ()
  ) funcs;
  arr

(** Export to JavaScript global *)
let () =
  Js.export "CELEvaluator"
    (object%js
       method evaluate expr ctx = evaluate_expr expr ctx
       method interpolate template ctx = interpolate template ctx
       method evaluateGrid expr gridSize time = evaluate_grid expr gridSize time
       method compileToGLSL expr gridSize = compile_to_glsl expr gridSize
       method generateAssetId assetType = generate_asset_id assetType
       method version = version ()
       method supportedFunctions = supported_functions ()
    end)
