(** Data Flow Functions for CEL Evaluator

    Functions that control how state updates are processed.
    These return metadata that the Svelte runtime interprets.

    Current functions:
    - local(value)  - Keep in local state only, no CRDT persistence
    - commit(value) - Persist to CRDT layer

    Future functions (designed for extensibility):
    - stream(value)         - Real-time streaming to peers
    - broadcast(value)      - One-time broadcast to all peers
    - sign(value)           - Cryptographically sign data
    - encrypt(value, key)   - Encrypt for specific peer
    - backend(endpoint, v)  - Send to backend for processing
    - throttle(value, ms)   - Rate limit updates
*)

open Cel_types

(** Data flow functions module *)
module Dataflow_funcs = struct
  (** local(value) - Keep value in local Svelte state only, no CRDT persistence.
      Returns: { __dataflow: "local", value: <the value> } *)
  let local_fn args _ctx _eval =
    match args with
    | [v] -> VMap [
        (VString "__dataflow", VString "local");
        (VString "value", v)
      ]
    | _ -> raise (Type_error "local(value)")

  (** commit(value) - Persist value to CRDT layer.
      Returns: { __dataflow: "commit", value: <the value> } *)
  let commit_fn args _ctx _eval =
    match args with
    | [v] -> VMap [
        (VString "__dataflow", VString "commit");
        (VString "value", v)
      ]
    | _ -> raise (Type_error "commit(value)")
end

(** Get all data flow functions for registry *)
let get_dataflow_functions () : func list = [
  { name = "local"; impl = Dataflow_funcs.local_fn };
  { name = "commit"; impl = Dataflow_funcs.commit_fn };
]
