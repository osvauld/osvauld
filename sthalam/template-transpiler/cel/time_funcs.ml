(** Time/Date Functions for CEL Evaluator

    Standard library date/time functions (now, timestamp, duration, format).
*)

open Cel_types

(** Date/time functions module *)
module Time_funcs = struct
  let now args _ctx _eval =
    match args with
    | [] -> VTimestamp (Unix.gettimeofday ())
    | _ -> raise (Type_error "now()")

  let timestamp args _ctx _eval =
    match args with
    | [] ->
        (* Return current Unix timestamp as integer (milliseconds) *)
        VInt (Int64.of_float (Unix.gettimeofday () *. 1000.0))
    | [VString s] ->
        (* Simple ISO 8601 parsing - can be enhanced *)
        (try
          (* Try parsing as Unix timestamp *)
          VTimestamp (float_of_string s)
        with Failure _ ->
          raise (Eval_error ("Cannot parse timestamp: " ^ s)))
    | _ -> raise (Type_error "timestamp() or timestamp(string)")

  let generate_id args _ctx _eval =
    match args with
    | [] ->
        (* Generate unique ID using timestamp + random component *)
        Random.self_init ();
        let ts = Int64.of_float (Unix.gettimeofday () *. 1000.0) in
        let rand = Random.int 999999 in
        VString (Printf.sprintf "%Ld-%06d" ts rand)
    | _ -> raise (Type_error "generateId()")

  let duration args _ctx _eval =
    match args with
    | [VString s] ->
        (* Parse duration strings like "1h", "30m", "45s" *)
        let len = String.length s in
        if len < 2 then
          raise (Eval_error "Invalid duration format")
        else
          let num_str = String.sub s 0 (len - 1) in
          let unit = s.[len - 1] in
          (try
            let num = float_of_string num_str in
            let seconds = match unit with
              | 's' -> num
              | 'm' -> num *. 60.0
              | 'h' -> num *. 3600.0
              | 'd' -> num *. 86400.0
              | _ -> raise (Eval_error ("Unknown duration unit: " ^ String.make 1 unit))
            in
            VDuration seconds
          with Failure _ ->
            raise (Eval_error ("Invalid duration number: " ^ num_str)))
    | _ -> raise (Type_error "duration(string)")

  let format args _ctx _eval =
    match args with
    | [VTimestamp t; VString _format] ->
        (* Simple formatting - just return ISO 8601 for now *)
        (* TODO: Implement proper date formatting *)
        let tm = Unix.gmtime t in
        let iso = Printf.sprintf "%04d-%02d-%02dT%02d:%02d:%02dZ"
          (tm.Unix.tm_year + 1900)
          (tm.Unix.tm_mon + 1)
          tm.Unix.tm_mday
          tm.Unix.tm_hour
          tm.Unix.tm_min
          tm.Unix.tm_sec
        in
        VString iso
    | _ -> raise (Type_error "format(timestamp, string)")
end

(** Get all time/date functions for registry *)
let get_time_functions () : func list = [
  { name = "now"; impl = Time_funcs.now };
  { name = "timestamp"; impl = Time_funcs.timestamp };
  { name = "generateId"; impl = Time_funcs.generate_id };
  { name = "duration"; impl = Time_funcs.duration };
  { name = "format"; impl = Time_funcs.format };
]
