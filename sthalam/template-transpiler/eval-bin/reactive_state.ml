(** Reactive State Management using OCaml React FRP *)

open React
open Cel.Cel_types

(** Signal types *)
type signal_info = {
  signal: value React.signal;
  setter: (value -> unit) option;
  dependencies: string list;
  expression: string option;
}

(** Global signal registry *)
let signals : (string, signal_info) Hashtbl.t = Hashtbl.create 32

(** Initialize source signal (state variable) *)
let create_source_signal (name : string) (initial_value : value) : unit =
  let signal, setter = S.create ~eq:(=) initial_value in
  let info = {
    signal;
    setter = Some setter;
    dependencies = [];
    expression = None;
  } in
  Hashtbl.add signals name info;
  Logging.WasmLog.debug (fun m -> m "Created source signal: %s" name)

(** Create computed signal with explicit dependencies *)
let create_computed_signal
    (name : string)
    (expr_str : string)
    (dep_names : string list) : unit =

  (* Get dependency signals *)
  let dep_signals = List.map (fun dep_name ->
    match Hashtbl.find_opt signals dep_name with
    | Some info -> info.signal
    | None -> failwith (Printf.sprintf "Unknown dependency: %s" dep_name)
  ) dep_names in

  (* Strip ${ } markers if present (same as evaluate_expr) *)
  let clean_expr =
    let trimmed = String.trim expr_str in
    if String.length trimmed > 3 &&
       String.sub trimmed 0 2 = "${" &&
       String.get trimmed (String.length trimmed - 1) = '}' then
      String.trim (String.sub trimmed 2 (String.length trimmed - 3))
    else
      trimmed
  in

  (* Parse CEL expression once *)
  let lexbuf = Lexing.from_string clean_expr in
  let expr = Cel.Cel_parser.main Cel.Cel_lexer.token lexbuf in

  (* Create evaluation function *)
  let eval_with_context dep_values =
    (* Build context from dependency names and values *)
    let context = List.map2 (fun name value -> (name, value)) dep_names dep_values in
    Cel.Cel_eval.eval context expr
  in

  (* Create computed signal based on number of dependencies *)
  let computed_signal = match dep_signals with
    | [] ->
        (* No dependencies - constant *)
        let result = Cel.Cel_eval.eval [] expr in
        S.const result
    | [single_dep] ->
        (* Single dependency - use S.map *)
        S.map (fun dep_value ->
          eval_with_context [dep_value]
        ) single_dep
    | [dep1; dep2] ->
        (* Two dependencies *)
        S.l2 (fun v1 v2 ->
          eval_with_context [v1; v2]
        ) dep1 dep2
    | [dep1; dep2; dep3] ->
        (* Three dependencies *)
        S.l3 (fun v1 v2 v3 ->
          eval_with_context [v1; v2; v3]
        ) dep1 dep2 dep3
    | [dep1; dep2; dep3; dep4] ->
        (* Four dependencies *)
        S.l4 (fun v1 v2 v3 v4 ->
          eval_with_context [v1; v2; v3; v4]
        ) dep1 dep2 dep3 dep4
    | [dep1; dep2; dep3; dep4; dep5] ->
        (* Five dependencies *)
        S.l5 (fun v1 v2 v3 v4 v5 ->
          eval_with_context [v1; v2; v3; v4; v5]
        ) dep1 dep2 dep3 dep4 dep5
    | [dep1; dep2; dep3; dep4; dep5; dep6] ->
        (* Six dependencies *)
        S.l6 (fun v1 v2 v3 v4 v5 v6 ->
          eval_with_context [v1; v2; v3; v4; v5; v6]
        ) dep1 dep2 dep3 dep4 dep5 dep6
    | _ ->
        (* More than 6 dependencies - not supported yet *)
        failwith (Printf.sprintf "Computed signal '%s' has %d dependencies (max 6 supported)"
          name (List.length dep_signals))
  in

  let info = {
    signal = computed_signal;
    setter = None;
    dependencies = dep_names;
    expression = Some expr_str;
  } in
  Hashtbl.add signals name info;
  Logging.WasmLog.debug (fun m -> m "Created computed signal: %s (deps: %s)"
    name (String.concat ", " dep_names))

(** Update signal value (triggers reactive propagation) *)
let update_signal (name : string) (new_value : value) : unit =
  match Hashtbl.find_opt signals name with
  | Some { setter = Some set_fn; _ } ->
      set_fn new_value;
      Logging.WasmLog.debug (fun m -> m "Updated signal: %s" name)

  | Some { setter = None; _ } ->
      Logging.WasmLog.warn (fun m -> m "Cannot update computed signal: %s" name)

  | None ->
      Logging.WasmLog.warn (fun m -> m "Unknown signal: %s" name)

(** Get current value of a signal *)
let get_signal_value (name : string) : value option =
  match Hashtbl.find_opt signals name with
  | Some info -> Some (S.value info.signal)
  | None -> None

(** Get all current values *)
let get_all_values () : (string * value) list =
  Hashtbl.fold (fun name info acc ->
    (name, S.value info.signal) :: acc
  ) signals []

(** Clear all signals *)
let clear_signals () : unit =
  Hashtbl.clear signals;
  Logging.WasmLog.info (fun m -> m "Cleared all signals")
