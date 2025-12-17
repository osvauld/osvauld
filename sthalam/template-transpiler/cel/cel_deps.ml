(** CEL Dependency Extraction

    Extracts variable dependencies from CEL expressions.
    Used by the HUML renderer for dependency-tracked reactivity.

    When a variable changes, only expressions that depend on it
    need to be re-evaluated.
*)

open Cel_types

module StringSet = Set.Make(String)

(** Extract all variable references from an expression.

    Walks the AST and collects all Ident nodes.
    Lambda parameters are excluded (they're local bindings).

    @param expr The CEL expression AST
    @return Set of variable names referenced
*)
let rec extract_deps (expr : expr) : StringSet.t =
  extract_deps_with_bound StringSet.empty expr

(** Internal: extract deps excluding bound variables (lambda params) *)
and extract_deps_with_bound (bound : StringSet.t) (expr : expr) : StringSet.t =
  match expr with
  (* Identifier - add if not bound by lambda *)
  | Ident name ->
      if StringSet.mem name bound then
        StringSet.empty
      else
        StringSet.singleton name

  (* Literals have no dependencies *)
  | Literal _ -> StringSet.empty

  (* Binary operations - union of both sides *)
  | Binary { left; right; _ } ->
      StringSet.union
        (extract_deps_with_bound bound left)
        (extract_deps_with_bound bound right)

  (* Unary operation *)
  | Unary { operand; _ } ->
      extract_deps_with_bound bound operand

  (* Member access - deps from object, field is string literal *)
  | Member { object_; _ } ->
      extract_deps_with_bound bound object_

  (* Index access - deps from both object and index *)
  | Index { object_; index } ->
      StringSet.union
        (extract_deps_with_bound bound object_)
        (extract_deps_with_bound bound index)

  (* Function call - deps from all arguments *)
  | Call { args; _ } ->
      List.fold_left
        (fun acc arg -> StringSet.union acc (extract_deps_with_bound bound arg))
        StringSet.empty
        args

  (* Method call - deps from object and arguments *)
  | MethodCall { object_; args; _ } ->
      let obj_deps = extract_deps_with_bound bound object_ in
      List.fold_left
        (fun acc arg -> StringSet.union acc (extract_deps_with_bound bound arg))
        obj_deps
        args

  (* List literal - deps from all elements *)
  | ListLit items ->
      List.fold_left
        (fun acc item -> StringSet.union acc (extract_deps_with_bound bound item))
        StringSet.empty
        items

  (* Map literal - deps from all keys and values *)
  | MapLit entries ->
      List.fold_left
        (fun acc (k, v) ->
          StringSet.union acc
            (StringSet.union
              (extract_deps_with_bound bound k)
              (extract_deps_with_bound bound v)))
        StringSet.empty
        entries

  (* Ternary - deps from condition and both branches *)
  | Ternary { condition; if_true; if_false } ->
      StringSet.union
        (extract_deps_with_bound bound condition)
        (StringSet.union
          (extract_deps_with_bound bound if_true)
          (extract_deps_with_bound bound if_false))

  (* Lambda - parameter is bound inside body *)
  | Lambda { param; body } ->
      let bound' = StringSet.add param bound in
      extract_deps_with_bound bound' body

(** Extract deps and return as list (sorted for determinism) *)
let extract_deps_list (expr : expr) : string list =
  extract_deps expr
  |> StringSet.elements
  |> List.sort String.compare

(** Extract deps from expression string.
    Parses the expression and extracts dependencies.

    @param expr_str Expression string (may be wrapped in ${ })
    @return List of variable names, or error
*)
let extract_deps_from_string (expr_str : string) : (string list, string) result =
  try
    (* Strip ${ } wrapper if present *)
    let expr_str = String.trim expr_str in
    let len = String.length expr_str in
    let clean_expr =
      if len >= 3 && String.sub expr_str 0 2 = "${" && expr_str.[len - 1] = '}' then
        String.trim (String.sub expr_str 2 (len - 3))
      else
        expr_str
    in
    let lexbuf = Lexing.from_string clean_expr in
    let ast = Cel_parser.main Cel_lexer.token lexbuf in
    Ok (extract_deps_list ast)
  with
  | Type_error msg -> Error ("Type error: " ^ msg)
  | Eval_error msg -> Error ("Eval error: " ^ msg)
  | Unknown_identifier name -> Error ("Unknown identifier: " ^ name)
  | Unknown_function name -> Error ("Unknown function: " ^ name)
  | exn -> Error (Printexc.to_string exn)

(** Extract deps from interpolation template string.
    Finds all {{ expr }} patterns and extracts their dependencies.

    @param template Template string with {{ expr }} patterns
    @return Combined list of all variable names, or error
*)
let extract_deps_from_template (template : string) : (string list, string) result =
  try
    let expr_regex = Str.regexp "{{\\([^}]+\\)}}" in
    let all_deps = ref StringSet.empty in
    let pos = ref 0 in

    (* Find all {{ expr }} patterns *)
    while
      try
        let _ = Str.search_forward expr_regex template !pos in
        true
      with Not_found -> false
    do
      let expr_str = Str.matched_group 1 template in
      let expr_str = String.trim expr_str in
      pos := Str.match_end ();

      (* Parse and extract deps from this expression *)
      let lexbuf = Lexing.from_string expr_str in
      let ast = Cel_parser.main Cel_lexer.token lexbuf in
      all_deps := StringSet.union !all_deps (extract_deps ast)
    done;

    Ok (StringSet.elements !all_deps |> List.sort String.compare)
  with
  | exn -> Error (Printexc.to_string exn)
