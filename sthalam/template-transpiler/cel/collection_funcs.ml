(** Collection Functions for CEL Evaluator

    Standard library collection manipulation functions (lists, maps).
    Includes higher-order functions with lambda support.
*)

open Cel_types

(** Collection functions module *)
module Collection_funcs = struct
  let size args _ctx _eval_lambda =
    match args with
    | [VString s] -> VInt (Int64.of_int (String.length s))
    | [VList items] -> VInt (Int64.of_int (List.length items))
    | [VMap entries] -> VInt (Int64.of_int (List.length entries))
    | [VBytes b] -> VInt (Int64.of_int (Bytes.length b))
    | _ -> raise (Type_error "size(string|list|map|bytes)")

  let filter args ctx eval_lambda =
    match args with
    | [VList items; VLambda lambda_expr] ->
        let filtered = List.filter (fun item ->
          let result = eval_lambda lambda_expr item ctx in
          is_truthy result
        ) items in
        VList filtered
    | _ -> raise (Type_error "filter(list, lambda)")

  let map args ctx eval_lambda =
    match args with
    | [VList items; VLambda lambda_expr] ->
        let mapped = List.map (fun item ->
          eval_lambda lambda_expr item ctx
        ) items in
        VList mapped
    | _ -> raise (Type_error "map(list, lambda)")

  let exists args ctx eval_lambda =
    match args with
    | [VList items; VLambda lambda_expr] ->
        let result = List.exists (fun item ->
          let result = eval_lambda lambda_expr item ctx in
          is_truthy result
        ) items in
        VBool result
    | _ -> raise (Type_error "exists(list, lambda)")

  let all args ctx eval_lambda =
    match args with
    | [VList items; VLambda lambda_expr] ->
        let result = List.for_all (fun item ->
          let result = eval_lambda lambda_expr item ctx in
          is_truthy result
        ) items in
        VBool result
    | _ -> raise (Type_error "all(list, lambda)")

  let flatten args _ctx _eval_lambda =
    match args with
    | [VList lists] ->
        let rec flatten_list = function
          | VList items -> List.concat_map flatten_list items
          | other -> [other]
        in
        VList (List.concat_map flatten_list lists)
    | _ -> raise (Type_error "flatten(list)")

  let unique args _ctx _eval_lambda =
    match args with
    | [VList items] ->
        let rec unique_list acc = function
          | [] -> List.rev acc
          | x :: xs ->
              if List.exists (values_equal x) acc then
                unique_list acc xs
              else
                unique_list (x :: acc) xs
        in
        VList (unique_list [] items)
    | _ -> raise (Type_error "unique(list)")

  let slice args _ctx _eval_lambda =
    match args with
    | [VList items; VInt start; VInt end_] ->
        let start_idx = Int64.to_int start in
        let end_idx = Int64.to_int end_ in
        let len = List.length items in
        if start_idx < 0 || start_idx > len then
          raise (Eval_error "slice start index out of bounds")
        else if end_idx < start_idx || end_idx > len then
          raise (Eval_error "slice end index out of bounds")
        else
          let rec take n lst = match (n, lst) with
            | 0, _ | _, [] -> []
            | n, x :: xs -> x :: take (n - 1) xs
          in
          let rec drop n lst = match (n, lst) with
            | 0, _ -> lst
            | _, [] -> []
            | n, _ :: xs -> drop (n - 1) xs
          in
          VList (take (end_idx - start_idx) (drop start_idx items))
    | _ -> raise (Type_error "slice(list, int, int)")

  let exists_one args ctx eval_lambda =
    match args with
    | [VList items; VLambda lambda_expr] ->
        let matches = List.filter (fun item ->
          let result = eval_lambda lambda_expr item ctx in
          is_truthy result
        ) items in
        VBool (List.length matches = 1)
    | _ -> raise (Type_error "exists_one(list, lambda)")

  let find args ctx eval_lambda =
    match args with
    | [VList items; VLambda lambda_expr] ->
        (match List.find_opt (fun item ->
          let result = eval_lambda lambda_expr item ctx in
          is_truthy result
        ) items with
        | Some item -> item
        | None -> VNull)
    | _ -> raise (Type_error "find(list, lambda)")

  let join args _ctx _eval_lambda =
    match args with
    | [VList items; VString separator] ->
        let strings = List.map (fun item ->
          match item with
          | VString s -> s
          | _ -> value_to_string item
        ) items in
        VString (String.concat separator strings)
    | _ -> raise (Type_error "join(list, string)")
end

(** Get all collection functions for registry *)
let get_collection_functions () : func list = [
  { name = "size"; impl = Collection_funcs.size };
  { name = "filter"; impl = Collection_funcs.filter };
  { name = "map"; impl = Collection_funcs.map };
  { name = "exists"; impl = Collection_funcs.exists };
  { name = "all"; impl = Collection_funcs.all };
  { name = "flatten"; impl = Collection_funcs.flatten };
  { name = "unique"; impl = Collection_funcs.unique };
  { name = "slice"; impl = Collection_funcs.slice };
  { name = "exists_one"; impl = Collection_funcs.exists_one };
  { name = "find"; impl = Collection_funcs.find };
  { name = "join"; impl = Collection_funcs.join };
]
