open Cel_types
open Cel_eval

let test_expr expr_str ctx =
  try
    let lexbuf = Lexing.from_string expr_str in
    let expr = Cel_parser.main Cel_lexer.token lexbuf in
    let result = eval ctx expr in
    Printf.printf "✓ Expression: %s\n" expr_str;
    Printf.printf "  Result: %s\n" (value_to_string result);
    Printf.printf "  Type: %s\n\n" (match result with
      | VString _ -> "string"
      | VBool _ -> "boolean"
      | VInt _ -> "int"
      | VFloat _ -> "float"
      | _ -> "other")
  with
  | e -> Printf.printf "✗ ERROR: %s - %s\n\n" expr_str (Printexc.to_string e)

let () =
  Printf.printf "=== Testing Ternary Operator ===\n\n";
  
  let ctx = [("selectedPattern", VString "wave")] in
  
  (* Test 1: Simple ternary with strings *)
  test_expr "selectedPattern == 'wave' ? 'hello' : 'world'" ctx;
  
  (* Test 2: Nested ternary *)
  test_expr "selectedPattern == 'wave' ? 'first' : selectedPattern == 'ripple' ? 'second' : 'third'" ctx;
  
  (* Test 3: With ripple *)
  let ctx2 = [("selectedPattern", VString "ripple")] in
  test_expr "selectedPattern == 'wave' ? 'first' : selectedPattern == 'ripple' ? 'second' : 'third'" ctx2;
  
  (* Test 4: The actual pattern expression (simplified) *)
  test_expr "selectedPattern == 'wave' ? 'sin(x)' : 'cos(x)'" ctx;
