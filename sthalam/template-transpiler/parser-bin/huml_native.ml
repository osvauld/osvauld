(** Native HUML parser - stdin/stdout JSON interface *)

open Huml

(** Convert HUML AST to Yojson for JSON output *)
let rec ast_to_yojson ast =
  match ast with
  | `String s -> `String s
  | `Float f -> `Float f
  | `Int i -> `Int i
  | `Intlit s -> `String s  (* Large integers as strings *)
  | `Bool b -> `Bool b
  | `Null -> `Null
  | `Assoc obj -> `Assoc (List.map (fun (k, v) -> (k, ast_to_yojson v)) obj)
  | `List lst -> `List (List.map ast_to_yojson lst)

(** Main entry point - read HUML from stdin, write JSON to stdout *)
let () =
  let input = In_channel.input_all stdin in
  let lexbuf = Lexing.from_string input in
  match parse lexbuf with
  | Ok ast ->
      let json = ast_to_yojson ast in
      let result = `Assoc [
        ("success", `Bool true);
        ("result", json);
        ("error", `Null)
      ] in
      print_string (Yojson.Safe.to_string result)
  | Error err_msg ->
      let result = `Assoc [
        ("success", `Bool false);
        ("result", `Null);
        ("error", `String err_msg)
      ] in
      print_string (Yojson.Safe.to_string result)
