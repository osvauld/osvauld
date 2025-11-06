(** Standard Library Function Registry

    Central registry for all CEL standard library functions.
    This module consolidates functions from all category modules:
    - String functions (9)
    - Collection functions (11)
    - Type conversion functions (3)
    - Time/date functions (5)
    - Math functions (22)
    - Geometry functions (8)

    Total: 58 functions

    PERFORMANCE: Uses hash table for O(1) function lookups instead of O(n) list traversal.
    Critical for canvas rendering with 30,000+ function calls per frame.
*)

open Cel_types

(** Get all standard library functions *)
let get_all_functions () : func list =
  List.concat [
    String_funcs.get_string_functions ();
    Collection_funcs.get_collection_functions ();
    Type_funcs.get_type_functions ();
    Time_funcs.get_time_functions ();
    Math_funcs.get_math_functions ();
    Geometry_funcs.get_geometry_functions ();
  ]

(** Hash table for O(1) function lookups - created once, reused forever *)
let function_table : (string, func) Hashtbl.t =
  let tbl = Hashtbl.create 64 in
  List.iter (fun f -> Hashtbl.add tbl f.name f) (get_all_functions ());
  tbl

(** Look up function by name - O(1) hash table lookup *)
let lookup_function (name : string) : func option =
  Hashtbl.find_opt function_table name

(** Get list of all function names (useful for debugging/introspection) *)
let get_function_names () : string list =
  List.map (fun f -> f.name) (get_all_functions ())

(** Get function count by category *)
let get_function_count () : int =
  List.length (get_all_functions ())
