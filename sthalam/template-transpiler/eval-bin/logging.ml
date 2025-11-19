(** Logging Infrastructure for CEL WASM

    Provides browser console logging for the CEL evaluator using OCaml Logs library.
    Supports multiple log levels and separate logging sources for different concerns.
*)

open Js_of_ocaml
open Cel.Cel_types

(* ============================================================================ *)
(* Logging Sources *)
(* ============================================================================ *)

(** Main WASM interface logging *)
let wasm_src = Logs.Src.create "cel.wasm" ~doc:"CEL WASM interface logging"
module WasmLog = (val Logs.src_log wasm_src : Logs.LOG)

(** JS/OCaml value conversion logging *)
let conversion_src = Logs.Src.create "cel.wasm.conversion" ~doc:"JS/OCaml value conversion logging"
module ConvLog = (val Logs.src_log conversion_src : Logs.LOG)

(* ============================================================================ *)
(* Logging Utilities *)
(* ============================================================================ *)

(** Get type name for logging *)
let type_name (v : value) : string =
  match v with
  | VNull -> "VNull"
  | VBool _ -> "VBool"
  | VInt _ -> "VInt"
  | VUint _ -> "VUint"
  | VFloat _ -> "VFloat"
  | VString _ -> "VString"
  | VBytes _ -> "VBytes"
  | VList _ -> "VList"
  | VMap _ -> "VMap"
  | VTimestamp _ -> "VTimestamp"
  | VDuration _ -> "VDuration"
  | VLambda _ -> "VLambda"

(** Get value string representation for logging *)
let value_string (v : value) : string =
  match v with
  | VNull -> "null"
  | VBool b -> string_of_bool b
  | VInt i -> Int64.to_string i
  | VUint u -> Int64.to_string u ^ "u"
  | VFloat f -> string_of_float f
  | VString s -> Printf.sprintf "\"%s\"" s
  | VList _ -> "[...]"
  | VMap _ -> "{...}"
  | VBytes b -> Printf.sprintf "<bytes:%d>" (Bytes.length b)
  | VTimestamp t -> Printf.sprintf "<timestamp:%f>" t
  | VDuration d -> Printf.sprintf "<duration:%f>" d
  | VLambda _ -> "<lambda>"

(* ============================================================================ *)
(* Initialization and Control *)
(* ============================================================================ *)

(** Initialize browser console reporter *)
let init () =
  Logs.set_reporter (Logs_browser.console_reporter ());
  (* Default to Info level, can be changed via JavaScript *)
  Logs.set_level (Some Logs.Info);
  WasmLog.info (fun m -> m "CEL WASM module initialized with logging")

(** Set log level from JavaScript
    @param level_js "debug" | "info" | "warning" | "error" | "app" | "none"
*)
let set_level (level_js : Js.js_string Js.t) =
  let level_str = String.lowercase_ascii (Js.to_string level_js) in
  let level = match level_str with
    | "debug" -> Some Logs.Debug
    | "info" -> Some Logs.Info
    | "warning" -> Some Logs.Warning
    | "error" -> Some Logs.Error
    | "app" -> Some Logs.App
    | "none" -> None
    | _ -> Some Logs.Info  (* Default to Info for invalid inputs *)
  in
  Logs.set_level level;
  WasmLog.info (fun m -> m "Log level set to: %s" level_str)
