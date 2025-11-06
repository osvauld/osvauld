open Cel.Cel_types
open Cel.Cel_eval

(* Helper to parse and evaluate *)
let test_expr expr_str expected_str =
  try
    let lexbuf = Lexing.from_string expr_str in
    let expr = Cel.Cel_parser.main Cel.Cel_lexer.token lexbuf in
    let result = eval [] expr in
    let result_str = value_to_string result in
    if result_str = expected_str then
      Printf.printf "✓ PASS: %s = %s\n" expr_str result_str
    else
      Printf.printf "✗ FAIL: %s\n  Expected: %s\n  Got: %s\n" expr_str expected_str result_str
  with
  | Type_error msg -> Printf.printf "✗ TYPE ERROR: %s - %s\n" expr_str msg
  | Eval_error msg -> Printf.printf "✗ EVAL ERROR: %s - %s\n" expr_str msg
  | e -> Printf.printf "✗ ERROR: %s - %s\n" expr_str (Printexc.to_string e)

let () =
  Printf.printf "\n=== CEL Standard Library Tests ===\n\n";

  (* Arithmetic *)
  Printf.printf "--- Arithmetic ---\n";
  test_expr "1 + 2 * 3" "7";
  test_expr "10 / 2 - 3" "2";
  test_expr "(5 + 3) * 2" "16";

  (* Comparison *)
  Printf.printf "\n--- Comparison ---\n";
  test_expr "5 > 3" "true";
  test_expr "2 == 2" "true";
  test_expr "1 != 2" "true";

  (* String functions *)
  Printf.printf "\n--- String Functions ---\n";
  test_expr "\"hello\".contains(\"ell\")" "true";
  test_expr "\"world\".startsWith(\"wor\")" "true";
  test_expr "\"  trim  \".trim()" "\"trim\"";
  test_expr "\"Hello\".toLowerCase()" "\"hello\"";

  (* List functions *)
  Printf.printf "\n--- List Functions ---\n";
  test_expr "[1, 2, 3].size()" "3";
  test_expr "\"hello\".size()" "5";

  (* Higher-order functions with lambdas *)
  Printf.printf "\n--- Higher-Order Functions ---\n";
  test_expr "[1, 2, 3, 4, 5].filter(x => x > 2)" "[3, 4, 5]";
  test_expr "[1, 2, 3].map(x => x * 2)" "[2, 4, 6]";
  test_expr "[1, 2, 3].exists(x => x == 2)" "true";
  test_expr "[1, 2, 3].all(x => x > 0)" "true";

  (* Sthalam-specific functions *)
  Printf.printf "\n--- Sthalam Extensions ---\n";
  test_expr "[1, 2, 3].exists_one(x => x == 2)" "true";
  test_expr "[1, 2, 2].exists_one(x => x == 2)" "false";
  test_expr "[1, 2, 3].find(x => x > 1)" "2";
  test_expr "[1, 2, 3].find(x => x > 10)" "null";
  test_expr "[\"hello\", \"world\"].join(\" \")" "\"hello world\"";
  test_expr "[1, 2, 3].join(\", \")" "\"1, 2, 3\"";

  (* Type conversions *)
  Printf.printf "\n--- Type Conversions ---\n";
  test_expr "int(\"42\")" "42";
  test_expr "double(3)" "3.";
  test_expr "string(123)" "\"123\"";

  (* Ternary *)
  Printf.printf "\n--- Ternary ---\n";
  test_expr "true ? \"yes\" : \"no\"" "\"yes\"";
  test_expr "5 > 3 ? 10 : 20" "10";

  Printf.printf "\n=== All tests complete! ===\n"
