(** Quick test for new math and geometry functions *)

open Cel.Cel_types
open Cel.Cel_eval

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
  Printf.printf "=== Testing New Math Functions ===\n\n";

  Printf.printf "--- Trigonometry ---\n";
  test_expr "sin(0)" "0.";
  test_expr "cos(0)" "1.";
  test_expr "tan(0)" "0.";
  test_expr "atan2(0, 1)" "0.";

  Printf.printf "\n--- Power & Root ---\n";
  test_expr "sqrt(4)" "2.";
  test_expr "sqrt(9)" "3.";
  test_expr "pow(2, 3)" "8.";
  test_expr "pow(10, 2)" "100.";

  Printf.printf "\n--- Rounding ---\n";
  test_expr "floor(3.7)" "3.";
  test_expr "ceil(3.2)" "4.";
  test_expr "round(3.5)" "4.";
  test_expr "abs(-5)" "5";
  test_expr "sign(-10)" "-1";
  test_expr "sign(10)" "1";
  test_expr "sign(0)" "0";

  Printf.printf "\n--- Comparison ---\n";
  test_expr "min(5, 3)" "3.";
  test_expr "max(5, 3)" "5.";
  test_expr "min(1, 2, 3)" "1.";
  test_expr "max(1, 2, 3)" "3.";

  Printf.printf "\n--- Constants ---\n";
  (* Just check they're non-zero *)
  let lexbuf1 = Lexing.from_string "pi()" in
  let pi_expr = Cel.Cel_parser.main Cel.Cel_lexer.token lexbuf1 in
  let pi_result = eval [] pi_expr in
  (match pi_result with
   | VFloat f when f > 3.0 && f < 3.2 -> Printf.printf "✓ PASS: pi() ≈ 3.14159\n"
   | _ -> Printf.printf "✗ FAIL: pi() value incorrect\n");

  let lexbuf2 = Lexing.from_string "e()" in
  let e_expr = Cel.Cel_parser.main Cel.Cel_lexer.token lexbuf2 in
  let e_result = eval [] e_expr in
  (match e_result with
   | VFloat f when f > 2.6 && f < 2.8 -> Printf.printf "✓ PASS: e() ≈ 2.71828\n"
   | _ -> Printf.printf "✗ FAIL: e() value incorrect\n");

  Printf.printf "\n=== Testing New Geometry Functions ===\n\n";

  Printf.printf "--- Distance & Interpolation ---\n";
  test_expr "distance(0, 0, 3, 4)" "5.";
  test_expr "lerp(0, 10, 0.5)" "5.";
  test_expr "clamp(15, 0, 10)" "10.";
  test_expr "clamp(-5, 0, 10)" "0.";
  test_expr "clamp(5, 0, 10)" "5.";

  Printf.printf "\n--- Angle Conversion ---\n";
  test_expr "degrees(0)" "0.";
  test_expr "radians(0)" "0.";
  let lexbuf3 = Lexing.from_string "degrees(3.14159)" in
  let deg180_expr = Cel.Cel_parser.main Cel.Cel_lexer.token lexbuf3 in
  let deg180 = eval [] deg180_expr in
  (match deg180 with
   | VFloat f when f > 179.0 && f < 181.0 -> Printf.printf "✓ PASS: degrees(π) ≈ 180\n"
   | _ -> Printf.printf "✗ FAIL: degrees(π) conversion incorrect\n");

  Printf.printf "\n--- Canvas Pattern Expression ---\n";
  (* Test a canvas-style expression with math functions *)
  let ctx = [("x", VFloat 10.0); ("time", VFloat 0.5)] in
  let lexbuf4 = Lexing.from_string "sin(x * 0.2 + time)" in
  let canvas_expr = Cel.Cel_parser.main Cel.Cel_lexer.token lexbuf4 in
  let result = eval ctx canvas_expr in
  (match result with
   | VFloat _ -> Printf.printf "✓ PASS: Canvas pattern expression compiles and evaluates\n"
   | _ -> Printf.printf "✗ FAIL: Canvas pattern expression failed\n");

  Printf.printf "\n=== All new function tests complete! ===\n";

  Printf.printf "\n=== Testing Ternary Operator ===\n\n";

  let test_ternary expr_str ctx expected_str =
    try
      let lexbuf = Lexing.from_string expr_str in
      let expr = Cel.Cel_parser.main Cel.Cel_lexer.token lexbuf in
      let result = eval ctx expr in
      let result_str = value_to_string result in
      if result_str = expected_str then
        Printf.printf "✓ PASS: %s = %s\n" expr_str result_str
      else
        Printf.printf "✗ FAIL: %s\n  Expected: %s\n  Got: %s\n" expr_str expected_str result_str
    with
    | Type_error msg -> Printf.printf "✗ TYPE ERROR: %s - %s\n" expr_str msg
    | Eval_error msg -> Printf.printf "✗ EVAL ERROR: %s - %s\n" expr_str msg
    | e -> Printf.printf "✗ ERROR: %s - %s\n" expr_str (Printexc.to_string e)
  in

  let ctx = [("selectedPattern", VString "wave")] in
  let ctx2 = [("selectedPattern", VString "ripple")] in

  (* Test 1: Simple ternary with strings *)
  test_ternary "selectedPattern == 'wave' ? 'hello' : 'world'" ctx "\"hello\"";

  (* Test 2: Nested ternary with wave selected *)
  test_ternary "selectedPattern == 'wave' ? 'first' : selectedPattern == 'ripple' ? 'second' : 'third'" ctx "\"first\"";

  (* Test 3: Nested ternary with ripple selected *)
  test_ternary "selectedPattern == 'wave' ? 'first' : selectedPattern == 'ripple' ? 'second' : 'third'" ctx2 "\"second\"";

  (* Test 4: The actual pattern expression (simplified) *)
  test_ternary "selectedPattern == 'wave' ? 'sin(x)' : 'cos(x)'" ctx "\"sin(x)\"";
  test_ternary "selectedPattern == 'wave' ? 'sin(x)' : 'cos(x)'" ctx2 "\"cos(x)\""
