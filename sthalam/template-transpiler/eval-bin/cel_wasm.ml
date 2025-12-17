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

(* Import logging infrastructure *)
module WasmLog = Logging.WasmLog
module ConvLog = Logging.ConvLog

(** Convert JavaScript value to CEL value *)
let rec js_to_value (js_val : Js.Unsafe.any) : value =
  let js_type = Js.to_string (Js.typeof js_val) in

  ConvLog.debug (fun m -> m "js_to_value: type=%s" js_type);

  match js_type with
  | "undefined" ->
      ConvLog.debug (fun m -> m "  -> VNull (undefined)");
      VNull
  | "boolean" ->
      let b = Js.to_bool (Js.Unsafe.coerce js_val) in
      ConvLog.debug (fun m -> m "  -> VBool %b" b);
      VBool b
  | "number" ->
      let num = float_of_js (Js.Unsafe.coerce js_val) in
      if Float.is_integer num then begin
        let i = Int64.of_float num in
        ConvLog.debug (fun m -> m "  -> VInt %Ld" i);
        VInt i
      end else begin
        ConvLog.debug (fun m -> m "  -> VFloat %f" num);
        VFloat num
      end
  | "string" ->
      let s = Js.to_string (Js.Unsafe.coerce js_val) in
      ConvLog.debug (fun m -> m "  -> VString \"%s\"" s);
      VString s
  | "object" ->
      if Js.Unsafe.equals js_val Js.null then begin
        ConvLog.debug (fun m -> m "  -> VNull (null)");
        VNull
      end else if Js.to_bool (Js.Unsafe.fun_call
                           (Js.Unsafe.js_expr "Array.isArray")
                           [| js_val |]) then begin
        (* JavaScript array → CEL list *)
        let arr : Js.Unsafe.any Js.js_array Js.t = Js.Unsafe.coerce js_val in
        let length = arr##.length in
        ConvLog.debug (fun m -> m "  -> VList (converting JS array, length=%d)" length);
        let items = ref [] in
        for i = length - 1 downto 0 do
          let elem = Js.array_get arr i in
          match Js.Optdef.to_option elem with
          | Some v ->
              ConvLog.debug (fun m -> m "    [%d]: converting array element" i);
              items := js_to_value v :: !items
          | None ->
              ConvLog.warn (fun m -> m "    [%d]: undefined array element, skipping" i)
        done;
        VList !items
      end else begin
        (* JavaScript object → CEL map *)
        let keys = Js.object_keys js_val in
        let length = keys##.length in
        ConvLog.debug (fun m -> m "  -> VMap (converting JS object, %d keys)" length);
        let pairs = ref [] in
        for i = 0 to length - 1 do
          match Js.Optdef.to_option (Js.array_get keys i) with
          | Some key_js ->
              let key_str = Js.to_string key_js in
              let value_js = Js.Unsafe.get js_val key_str in
              ConvLog.debug (fun m -> m "    field[%d]: key=\"%s\" js_type=%s"
                i key_str (Js.to_string (Js.typeof value_js)));

              let cel_key = VString key_str in
              let cel_value = js_to_value value_js in

              (* Log the converted value for debugging *)
              ConvLog.debug (fun m -> m "    field[%d]: \"%s\" = %s (cel_type=%s)"
                i key_str (Logging.value_string cel_value) (Logging.type_name cel_value));

              pairs := (cel_key, cel_value) :: !pairs
          | None ->
              ConvLog.warn (fun m -> m "    field[%d]: undefined key, skipping" i)
        done;
        let result = VMap (List.rev !pairs) in
        ConvLog.debug (fun m -> m "  -> VMap with %d pairs created" (List.length !pairs));
        result
      end
  | _ ->
      ConvLog.warn (fun m -> m "  -> VNull (unknown type: %s)" js_type);
      VNull

and float_of_js (js_val : Js.Unsafe.any) : float =
  Js.float_of_number (Js.Unsafe.coerce js_val)

(** Convert CEL value back to JavaScript *)
let rec value_to_js (v : value) : Js.Unsafe.any =
  ConvLog.debug (fun m -> m "value_to_js: %s" (Logging.type_name v));

  match v with
  | VNull ->
      ConvLog.debug (fun m -> m "  -> JS null");
      Js.Unsafe.inject Js.null
  | VBool b ->
      ConvLog.debug (fun m -> m "  -> JS boolean %b" b);
      Js.Unsafe.inject (Js.bool b)
  | VInt i ->
      ConvLog.debug (fun m -> m "  -> JS number %Ld" i);
      Js.Unsafe.inject (Js.number_of_float (Int64.to_float i))
  | VUint u ->
      ConvLog.debug (fun m -> m "  -> JS number %Lu (uint)" u);
      Js.Unsafe.inject (Js.number_of_float (Int64.to_float u))
  | VFloat f ->
      ConvLog.debug (fun m -> m "  -> JS number %f" f);
      Js.Unsafe.inject (Js.number_of_float f)
  | VString s ->
      ConvLog.debug (fun m -> m "  -> JS string \"%s\"" s);
      Js.Unsafe.inject (Js.string s)
  | VBytes b ->
      ConvLog.debug (fun m -> m "  -> JS string (from bytes, length=%d)" (Bytes.length b));
      Js.Unsafe.inject (Js.string (Bytes.to_string b))
  | VList items ->
      ConvLog.debug (fun m -> m "  -> JS array (length=%d)" (List.length items));
      let arr = new%js Js.array_empty in
      List.iteri (fun idx item ->
        ConvLog.debug (fun m -> m "    [%d]: converting %s" idx (Logging.type_name item));
        let _ = arr##push (value_to_js item) in
        ()
      ) items;
      Js.Unsafe.inject arr
  | VMap pairs ->
      ConvLog.info (fun m -> m "  -> JS object (%d pairs)" (List.length pairs));
      let obj = Js.Unsafe.obj [||] in
      List.iteri (fun idx (k, v) ->
        match k with
        | VString key ->
            ConvLog.debug (fun m -> m "    field[%d]: setting \"%s\" = %s"
              idx key (Logging.type_name v));
            let js_val = value_to_js v in
            Js.Unsafe.set obj (Js.string key) js_val;

            (* Verify the value was set correctly *)
            let verify_js = Js.Unsafe.get obj (Js.string key) in
            let verify_type = Js.to_string (Js.typeof verify_js) in
            ConvLog.debug (fun m -> m "    field[%d]: verified \"%s\" type=%s defined=%b"
              idx key verify_type
              (not (Js.Unsafe.equals verify_js Js.undefined)));
        | _ ->
            ConvLog.warn (fun m -> m "    field[%d]: non-string key type %s, ignoring"
              idx (Logging.type_name k))
      ) pairs;
      ConvLog.info (fun m -> m "  -> JS object created with %d fields" (List.length pairs));
      Js.Unsafe.inject obj
  | VTimestamp t ->
      ConvLog.debug (fun m -> m "  -> JS number (timestamp %f)" t);
      Js.Unsafe.inject (Js.number_of_float t)
  | VDuration d ->
      ConvLog.debug (fun m -> m "  -> JS number (duration %f)" d);
      Js.Unsafe.inject (Js.number_of_float d)
  | VLambda _ ->
      ConvLog.debug (fun m -> m "  -> JS string \"<lambda>\"");
      Js.Unsafe.inject (Js.string "<lambda>")

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

  WasmLog.info (fun m -> m "evaluate_expr: expression=\"%s\"" expr_str);

  try
    (* Strip ${ } markers if present (pure expression syntax) *)
    let clean_expr =
      let trimmed = String.trim expr_str in
      if String.length trimmed > 3 &&
         String.sub trimmed 0 2 = "${" &&
         String.get trimmed (String.length trimmed - 1) = '}' then begin
        let cleaned = String.trim (String.sub trimmed 2 (String.length trimmed - 3)) in
        WasmLog.debug (fun m -> m "  Stripped ${ } markers: \"%s\"" cleaned);
        cleaned
      end else
        trimmed
    in

    (* Parse expression *)
    WasmLog.debug (fun m -> m "  Parsing expression...");
    let lexbuf = Lexing.from_string clean_expr in
    let ast = Cel_parser.main Cel_lexer.token lexbuf in
    WasmLog.debug (fun m -> m "  Parse successful");

    (* Create context from JS object (lazy conversion) *)
    WasmLog.debug (fun m -> m "  Converting context from JS object...");
    let context = context_from_js_object context_obj in
    WasmLog.debug (fun m -> m "  Context has %d bindings" (List.length context));

    (* Evaluate *)
    WasmLog.debug (fun m -> m "  Evaluating AST...");
    let result = eval context ast in
    WasmLog.info (fun m -> m "  Evaluation result: %s" (Logging.type_name result));

    (* Convert result back to JavaScript *)
    WasmLog.debug (fun m -> m "  Converting result to JS...");
    let js_result = value_to_js result in
    WasmLog.info (fun m -> m "  SUCCESS: expression evaluated");

    object%js
      val success = Js.bool true
      val result = js_result
      val error = Js.Unsafe.inject Js.null
    end
  with
  | Cel.Cel_types.Type_error msg ->
      WasmLog.err (fun m -> m "Type error: %s" msg);
      object%js
        val success = Js.bool false
        val result = Js.Unsafe.inject Js.null
        val error = Js.Unsafe.inject (Js.string ("Type error: " ^ msg))
      end
  | Cel.Cel_types.Eval_error msg ->
      WasmLog.err (fun m -> m "Evaluation error: %s" msg);
      object%js
        val success = Js.bool false
        val result = Js.Unsafe.inject Js.null
        val error = Js.Unsafe.inject (Js.string ("Evaluation error: " ^ msg))
      end
  | Cel.Cel_types.Unknown_identifier msg ->
      WasmLog.err (fun m -> m "Unknown identifier: %s" msg);
      object%js
        val success = Js.bool false
        val result = Js.Unsafe.inject Js.null
        val error = Js.Unsafe.inject (Js.string ("Unknown identifier: " ^ msg))
      end
  | Cel.Cel_types.Unknown_function msg ->
      WasmLog.err (fun m -> m "Unknown function: %s" msg);
      object%js
        val success = Js.bool false
        val result = Js.Unsafe.inject Js.null
        val error = Js.Unsafe.inject (Js.string ("Unknown function: " ^ msg))
      end
  | exn ->
      let error_msg = Printexc.to_string exn in
      WasmLog.err (fun m -> m "PARSE ERROR: %s" error_msg);
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
  WasmLog.debug (fun m -> m "Interpolating template (length %d): %s" (String.length template)
    (if String.length template > 100 then String.sub template 0 100 ^ "..." else template));

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
      | exn ->
          WasmLog.warn (fun m -> m "Interpolation failed for '{{%s}}': %s" expr_str (Printexc.to_string exn));
          matched  (* Keep original on error *)
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
    (* Data flow functions (2) *)
    "local"; "commit";
  |] in
  let arr = new%js Js.array_empty in
  Array.iter (fun name ->
    let _ = arr##push (Js.string name) in
    ()
  ) funcs;
  arr

(** ==== REACTIVE SYSTEM ==== *)

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

(** Initialize reactive system from HUML template
    @param template_js JavaScript object with documents.publisherState and documents.publisherComputed
*)
let init_reactive_template (template_js : Js.Unsafe.any) : unit =
  WasmLog.info (fun m -> m "Initializing reactive template");

  (* Clear existing state *)
  Reactive_state.clear_signals ();

  (* Get documents object from template *)
  let documents_opt = Js.Unsafe.get template_js "documents" in
  let has_documents = Js.Optdef.test documents_opt in

  if not has_documents then begin
    WasmLog.warn (fun m -> m "No 'documents' field in template, skipping reactive initialization");
    ()
  end else begin
    let documents = documents_opt in

    (* 1. Create source signals from all fields in documents:: (v2.0 structure) *)
    let doc_keys = Js.object_keys documents in
    WasmLog.info (fun m -> m "  Processing v2.0 document fields (found %d fields)" doc_keys##.length);

    for i = 0 to doc_keys##.length - 1 do
      match Js.Optdef.to_option (Js.array_get doc_keys i) with
      | Some key_js ->
          let key = Js.to_string key_js in
          WasmLog.debug (fun m -> m "  Processing field: %s" key);
          let field_config = Js.Unsafe.get documents key in
          let is_object = Js.to_string (Js.typeof field_config) = "object" in

          if is_object then begin
            let initial_opt = Js.Unsafe.get field_config "initial" in
            let has_initial = Js.Optdef.test initial_opt in

            if has_initial then begin
              let initial_js = Js.Unsafe.get field_config "initial" in
              let initial_value = js_to_value initial_js in
              Reactive_state.create_source_signal key initial_value;
              WasmLog.info (fun m -> m "  Created source signal: %s = %s"
                key (Logging.value_string initial_value))
            end else begin
              WasmLog.debug (fun m -> m "  Field '%s' has no 'initial' property, skipping" key)
            end
          end
      | None -> ()
    done;

    (* 2. Create computed signals from template.computed (v2.0 only) *)
    let computed_opt = Js.Unsafe.get template_js "computed" in
    let has_computed = Js.Optdef.test computed_opt in

    let process_computed_section () =
      if has_computed then begin
        let pub_computed = computed_opt in
        let section_name = "computed" in
        let comp_keys = Js.object_keys pub_computed in
        WasmLog.info (fun m -> m "  Found %s with %d keys" section_name comp_keys##.length);

        for i = 0 to comp_keys##.length - 1 do
          WasmLog.debug (fun m -> m "  Loop iteration %d" i);
          try
            match Js.Optdef.to_option (Js.array_get comp_keys i) with
            | Some key_js ->
                let key = Js.to_string key_js in
                WasmLog.debug (fun m -> m "  Processing computed field: %s" key);
              let computed_config = Js.Unsafe.get pub_computed key in

              (* Check if it's an object with expr and deps fields (v2.0 only) *)
              let is_object = Js.to_string (Js.typeof computed_config) = "object" in
              WasmLog.debug (fun m -> m "    is_object: %b" is_object);

              let expr_field_opt = Js.Unsafe.get computed_config "expr" in
              let deps_field_opt = Js.Unsafe.get computed_config "deps" in
              let has_expr = is_object && Js.Optdef.test expr_field_opt in
              let has_deps = is_object && Js.Optdef.test deps_field_opt in

              WasmLog.debug (fun m -> m "    has_expr: %b, has_deps: %b" has_expr has_deps);

              if has_expr && has_deps then begin
                (* Get expression string *)
                let expr_str = Js.to_string (Js.Unsafe.get computed_config "expr") in
                WasmLog.debug (fun m -> m "    Expression: %s" expr_str);

                (* Get dependencies array *)
                let deps_arr = Js.Unsafe.get computed_config "deps" in
                let deps = js_array_to_string_list deps_arr in
                WasmLog.debug (fun m -> m "    Dependencies: [%s]" (String.concat ", " deps));

                WasmLog.debug (fun m -> m "    Creating computed signal...");
                Reactive_state.create_computed_signal key expr_str deps;
                WasmLog.info (fun m -> m "  Created computed signal: %s (deps: %s)"
                  key (String.concat ", " deps))
              end else begin
                WasmLog.warn (fun m -> m "  Skipping computed value '%s': missing expr or deps fields" key)
              end
            | None ->
                WasmLog.warn (fun m -> m "  Skipped undefined key at index %d" i)
          with exn ->
            WasmLog.err (fun m -> m "  Exception processing computed field at index %d: %s" i (Printexc.to_string exn))
        done
      end
    in

    process_computed_section ();

    WasmLog.info (fun m -> m "Reactive template initialized")
  end

(** Update reactive state (triggers reactive propagation)
    @param name Signal name
    @param value_js New value (JavaScript)
*)
let update_reactive_state (name : Js.js_string Js.t) (value_js : Js.Unsafe.any) : unit =
  let name_str = Js.to_string name in
  let value = js_to_value value_js in
  Reactive_state.update_signal name_str value

(** Get current value of a reactive signal
    @param name Signal name
    @return Current value (JavaScript) or undefined
*)
let get_reactive_value (name : Js.js_string Js.t) : Js.Unsafe.any =
  let name_str = Js.to_string name in
  match Reactive_state.get_signal_value name_str with
  | Some value -> value_to_js value
  | None -> Js.Unsafe.inject Js.undefined

(** Get all current reactive values
    @return JavaScript object with all signal values
*)
let get_all_reactive_values () : Js.Unsafe.any =
  let all_values = Reactive_state.get_all_values () in
  let obj = Js.Unsafe.obj [||] in
  List.iter (fun (name, value) ->
    Js.Unsafe.set obj (Js.string name) (value_to_js value)
  ) all_values;
  Js.Unsafe.inject obj

(** Export to JavaScript global *)
let () =
  (* Initialize logging on module load *)
  Logging.init ();

  Js.export "CELEvaluator"
    (object%js
       method evaluate expr ctx = evaluate_expr expr ctx
       method interpolate template ctx = interpolate template ctx
       method evaluateGrid expr gridSize time = evaluate_grid expr gridSize time
       method compileToGLSL expr gridSize = compile_to_glsl expr gridSize
       method generateAssetId assetType = generate_asset_id assetType
       method setLogLevel level = Logging.set_level level
       method version = version ()
       method supportedFunctions = supported_functions ()
    end);

  Js.export "CELReactive"
    (object%js
       method initTemplate template = init_reactive_template template
       method updateState name value = update_reactive_state name value
       method getValue name = get_reactive_value name
       method getAllValues = get_all_reactive_values ()
       method setLogLevel level = Logging.set_level level
       method version = Js.string "1.0.0-reactive"
    end)
