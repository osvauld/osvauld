(** WASM bindings for HUML parser *)

open Js_of_ocaml
open Huml

(** Convert HUML AST to JavaScript object *)
let rec ast_to_js ast =
  match ast with
  | `String s -> Js.Unsafe.inject (Js.string s)
  | `Float f -> Js.Unsafe.inject (Js.number_of_float f)
  | `Int i -> Js.Unsafe.inject (Js.number_of_float (float_of_int i))
  | `Intlit s ->
      (* For large integers that don't fit in OCaml int, pass as string *)
      Js.Unsafe.inject (Js.string s)
  | `Bool b -> Js.Unsafe.inject (Js.bool b)
  | `Null -> Js.Unsafe.inject Js.null
  | `Assoc obj ->
      let js_obj = Js.Unsafe.obj [||] in
      List.iter
        (fun (key, value) ->
          let js_key = Js.string key in
          let js_value = ast_to_js value in
          Js.Unsafe.set js_obj js_key js_value)
        obj;
      Js.Unsafe.inject js_obj
  | `List lst ->
      let js_array = Js.array (Array.of_list (List.map ast_to_js lst)) in
      Js.Unsafe.inject js_array

(** Parse HUML string and return result *)
let parse_huml (huml_str : Js.js_string Js.t) =
  let str = Js.to_string huml_str in
  try
    let lexbuf = Lexing.from_string str in
    match parse lexbuf with
    | Ok ast ->
        let js_result = ast_to_js ast in
        object%js
          val success = Js.bool true
          val result = js_result
          val error = Js.Unsafe.inject Js.null
        end
    | Error err_msg ->
        object%js
          val success = Js.bool false
          val result = Js.Unsafe.inject Js.null
          val error = Js.Unsafe.inject (Js.string err_msg)
        end
  with exn ->
    let err_msg =
      Printf.sprintf "Unexpected error: %s" (Printexc.to_string exn)
    in
    object%js
      val success = Js.bool false
      val result = Js.Unsafe.inject Js.null
      val error = Js.Unsafe.inject (Js.string err_msg)
    end

(** Get supported HUML version *)
let get_version () = Js.string "v0.1.0"

(** Export HUMLParser global object *)
let _ =
  Js.export "HUMLParser"
    (object%js
       method parse huml_str = parse_huml huml_str
       method version = get_version ()
     end)
