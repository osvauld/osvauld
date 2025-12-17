(** CEL WASI Entry Point

    This module provides a WASI-compatible interface to the CEL evaluator.
    It reads JSON commands from stdin and writes JSON responses to stdout.

    Commands:
    - evaluate: Evaluate a CEL expression with a context
    - interpolate: Interpolate {{ expr }} patterns in a template string
    - extract_deps: Extract variable dependencies from a CEL expression
    - extract_template_deps: Extract dependencies from {{ expr }} patterns in a template

    Request format:
    {
      "cmd": "evaluate" | "interpolate" | "extract_deps" | "extract_template_deps",
      "expr": "expression string" (for evaluate/extract_deps),
      "template": "template string" (for interpolate/extract_template_deps),
      "context": { ... } (for evaluate/interpolate)
    }

    Response format:
    {
      "success": true | false,
      "result": <value> (on success),
      "deps": ["var1", "var2"] (for extract_deps commands),
      "error": "error message" (on failure)
    }
*)

open Cel.Cel_types
open Cel.Cel_eval
open Cel.Cel_deps

(** Convert Yojson.Safe.t to CEL value *)
let rec json_to_cel_value (json : Yojson.Safe.t) : value =
  match json with
  | `Null -> VNull
  | `Bool b -> VBool b
  | `Int n -> VInt (Int64.of_int n)
  | `Intlit s -> VInt (Int64.of_string s)
  | `Float f -> VFloat f
  | `String s -> VString s
  | `List items -> VList (List.map json_to_cel_value items)
  | `Assoc pairs ->
      VMap (List.map (fun (k, v) -> (VString k, json_to_cel_value v)) pairs)

(** Convert CEL value to Yojson.Safe.t *)
let rec cel_value_to_json (v : value) : Yojson.Safe.t =
  match v with
  | VNull -> `Null
  | VBool b -> `Bool b
  | VInt n -> `Intlit (Int64.to_string n)
  | VUint n -> `Intlit (Int64.to_string n)
  | VFloat f -> `Float f
  | VString s -> `String s
  | VBytes b -> `String (Bytes.to_string b)
  | VList items -> `List (List.map cel_value_to_json items)
  | VMap pairs ->
      `Assoc (List.filter_map (fun (k, v) ->
        match k with
        | VString key -> Some (key, cel_value_to_json v)
        | _ -> None
      ) pairs)
  | VTimestamp t -> `Float t
  | VDuration d -> `Float d
  | VLambda _ -> `String "<lambda>"

(** Build context from JSON object *)
let context_from_json (json : Yojson.Safe.t) : context =
  match json with
  | `Assoc pairs ->
      List.map (fun (k, v) -> (k, json_to_cel_value v)) pairs
  | _ -> []

(** Strip ${ } wrapper if present *)
let strip_expr_wrapper (expr : string) : string =
  let expr = String.trim expr in
  let len = String.length expr in
  if len >= 3 && String.sub expr 0 2 = "${" && expr.[len - 1] = '}' then
    String.trim (String.sub expr 2 (len - 3))
  else
    expr

(** Evaluate a CEL expression *)
let evaluate_expr (expr_str : string) (ctx : context) : Yojson.Safe.t =
  try
    let clean_expr = strip_expr_wrapper expr_str in
    let lexbuf = Lexing.from_string clean_expr in
    let ast = Cel.Cel_parser.main Cel.Cel_lexer.token lexbuf in
    let result = eval ctx ast in
    `Assoc [
      ("success", `Bool true);
      ("result", cel_value_to_json result)
    ]
  with
  | Type_error msg ->
      `Assoc [("success", `Bool false); ("error", `String ("Type error: " ^ msg))]
  | Eval_error msg ->
      `Assoc [("success", `Bool false); ("error", `String ("Eval error: " ^ msg))]
  | Unknown_identifier name ->
      `Assoc [("success", `Bool false); ("error", `String ("Unknown identifier: " ^ name))]
  | Unknown_function name ->
      `Assoc [("success", `Bool false); ("error", `String ("Unknown function: " ^ name))]
  | exn ->
      `Assoc [("success", `Bool false); ("error", `String (Printexc.to_string exn))]

(** Interpolate {{ expr }} patterns in template *)
let interpolate_template (template : string) (ctx : context) : Yojson.Safe.t =
  try
    let expr_regex = Str.regexp "{{\\([^}]+\\)}}" in
    let replace_expr matched =
      let expr_str = Str.matched_group 1 matched in
      let expr_str = String.trim expr_str in
      try
        let lexbuf = Lexing.from_string expr_str in
        let ast = Cel.Cel_parser.main Cel.Cel_lexer.token lexbuf in
        let result = eval ctx ast in
        match result with
        | VString s -> s
        | VNull -> ""
        | VBool b -> string_of_bool b
        | VInt n -> Int64.to_string n
        | VUint n -> Int64.to_string n
        | VFloat f ->
            if Float.is_integer f then
              Int64.to_string (Int64.of_float f)
            else
              string_of_float f
        | _ -> value_to_string result
      with _ -> matched
    in
    let result = Str.global_substitute expr_regex replace_expr template in
    `Assoc [
      ("success", `Bool true);
      ("result", `String result)
    ]
  with exn ->
    `Assoc [("success", `Bool false); ("error", `String (Printexc.to_string exn))]

(** Process a single command *)
let process_command (json : Yojson.Safe.t) : Yojson.Safe.t =
  let open Yojson.Safe.Util in
  try
    let cmd = json |> member "cmd" |> to_string in
    let ctx_json = json |> member "context" in
    let ctx = context_from_json ctx_json in

    match cmd with
    | "evaluate" ->
        let expr = json |> member "expr" |> to_string in
        evaluate_expr expr ctx
    | "interpolate" ->
        let template = json |> member "template" |> to_string in
        interpolate_template template ctx
    | "extract_deps" ->
        let expr = json |> member "expr" |> to_string in
        (match extract_deps_from_string expr with
        | Ok deps ->
            `Assoc [
              ("success", `Bool true);
              ("deps", `List (List.map (fun s -> `String s) deps))
            ]
        | Error msg ->
            `Assoc [("success", `Bool false); ("error", `String msg)])
    | "extract_template_deps" ->
        let template = json |> member "template" |> to_string in
        (match extract_deps_from_template template with
        | Ok deps ->
            `Assoc [
              ("success", `Bool true);
              ("deps", `List (List.map (fun s -> `String s) deps))
            ]
        | Error msg ->
            `Assoc [("success", `Bool false); ("error", `String msg)])
    | _ ->
        `Assoc [("success", `Bool false); ("error", `String ("Unknown command: " ^ cmd))]
  with
  | Yojson.Safe.Util.Type_error (msg, _) ->
      `Assoc [("success", `Bool false); ("error", `String ("Invalid request: " ^ msg))]
  | exn ->
      `Assoc [("success", `Bool false); ("error", `String (Printexc.to_string exn))]

(** Main WASI entry point *)
let () =
  try
    let input = In_channel.input_all In_channel.stdin in
    let request = Yojson.Safe.from_string input in
    let response = process_command request in
    print_string (Yojson.Safe.to_string response)
  with
  | Yojson.Json_error msg ->
      print_string (Yojson.Safe.to_string (
        `Assoc [("success", `Bool false); ("error", `String ("JSON parse error: " ^ msg))]
      ))
  | exn ->
      print_string (Yojson.Safe.to_string (
        `Assoc [("success", `Bool false); ("error", `String (Printexc.to_string exn))]
      ))
