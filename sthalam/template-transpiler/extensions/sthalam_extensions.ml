(** Sthalam Custom Extensions

    This module contains Sthalam-specific extensions to the CEL evaluator:
    - Loro integration for lazy loading
    - Custom functions for Sthalam use cases
    - Performance optimizations

    These extensions are SEPARATE from the core CEL implementation,
    allowing us to maintain a clean, standard CEL parser while
    adding custom behavior at evaluation time.
*)

open Cel_types

(** Custom value type for Loro collections *)
type loro_collection = {
  doc_id: string;           (* Loro document ID *)
  path: string;             (* Path to collection in document *)
  length: int;              (* Number of items (cached) *)
}

(** Extended value type including Loro *)
type extended_value =
  | CelValue of value                          (* Standard CEL value *)
  | LoroCollection of loro_collection          (* Lazy Loro collection *)
  | LoroObject of string * string              (* Loro object reference *)

(** TODO: Implement Loro integration

    When evaluating expressions like:
      posts.filter(p => p.author == currentUser)

    If `posts` is detected as a Loro collection, we should:
    1. NOT load all items into memory
    2. Create a lazy filtered view
    3. Return iterator/cursor for efficient access

    Example implementation:
    ```ocaml
    let eval_with_loro ctx expr =
      match expr with
      | Call { function_ = "filter"; args = [collection; predicate] } ->
          let coll_val = eval ctx collection in
          (match detect_loro coll_val with
          | Some loro_coll ->
              (* Create lazy filtered view *)
              create_loro_filter loro_coll predicate ctx
          | None ->
              (* Standard in-memory filter *)
              standard_filter coll_val predicate ctx)
      | _ -> eval ctx expr
    ```
*)

(** TODO: Custom Sthalam functions

    Add domain-specific functions that are useful for Sthalam templates:

    - formatDate(timestamp, format) - Format date/time
    - markdown(text) - Render markdown to HTML
    - truncate(text, length) - Truncate text with ellipsis
    - pluralize(count, singular, plural) - Pluralize words
    - avatar(userId) - Get avatar URL
    - ...etc
*)

(** Placeholder: Detect if value is a Loro collection *)
let is_loro_collection (v : value) : bool =
  (* TODO: Implement detection logic *)
  (* For now, return false - all values are standard CEL *)
  false

(** Placeholder: Evaluate with Loro optimizations *)
let eval_with_extensions (ctx : Cel_eval.context) (expr : expr) : value =
  (* For now, just use standard CEL evaluation *)
  (* TODO: Add Loro detection and lazy loading *)
  Cel_eval.eval ctx expr

(** Placeholder: Custom function registry *)
let custom_functions : (string * (value list -> value)) list = [
  (* Example: formatDate function *)
  (* ("formatDate", fun args ->
      match args with
      | [VTimestamp t; VString format] ->
          VString (format_timestamp t format)
      | _ -> raise (Type_error "formatDate(timestamp, string)"))
  *)
]

(** TODO: Register custom functions with evaluator *)
let register_custom_functions () : unit =
  (* TODO: Add custom functions to CEL evaluator *)
  ()
