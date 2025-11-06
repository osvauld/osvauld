{
(** CEL Lexer - Tokenizer for Common Expression Language

    Based on CEL spec: https://github.com/google/cel-spec

    Tokens:
    - Literals: int, uint, float, string, bytes, bool, null
    - Operators: arithmetic, comparison, logical
    - Symbols: (), [], {}, ., ,, ?, :
    - Keywords: true, false, null, in
*)

open Cel_parser

exception Lexer_error of string

(* Helper: unescape string *)
let unescape_string s =
  let buf = Buffer.create (String.length s) in
  let rec loop i =
    if i >= String.length s then Buffer.contents buf
    else
      match s.[i] with
      | '\\' when i + 1 < String.length s -> (
          match s.[i + 1] with
          | 'n' -> Buffer.add_char buf '\n'; loop (i + 2)
          | 't' -> Buffer.add_char buf '\t'; loop (i + 2)
          | 'r' -> Buffer.add_char buf '\r'; loop (i + 2)
          | '\\' -> Buffer.add_char buf '\\'; loop (i + 2)
          | '"' -> Buffer.add_char buf '"'; loop (i + 2)
          | '\'' -> Buffer.add_char buf '\''; loop (i + 2)
          | _ -> Buffer.add_char buf s.[i]; loop (i + 1))
      | c -> Buffer.add_char buf c; loop (i + 1)
  in
  loop 0
}

(* Character classes *)
let whitespace = [' ' '\t' '\r' '\n']
let digit = ['0'-'9']
let hex_digit = ['0'-'9' 'a'-'f' 'A'-'F']
let letter = ['a'-'z' 'A'-'Z']
let ident_char = letter | digit | '_'

(* Numeric literals *)
let dec_int = digit+
let hex_int = "0x" hex_digit+
let uint_lit = (dec_int | hex_int) ['u' 'U']
let exponent = ['e' 'E'] ['+' '-']? digit+
let float_lit = digit+ '.' digit* exponent?
              | digit+ exponent

(* String literals *)
let string_char = [^ '"' '\\']

rule token = parse
  (* Whitespace *)
  | whitespace+           { token lexbuf }

  (* Comments *)
  | "//" [^ '\n']*        { token lexbuf }

  (* Literals - Keywords *)
  | "true"                { TRUE }
  | "false"               { FALSE }
  | "null"                { NULL }

  (* Operators - Relational *)
  | "=="                  { EQ }
  | "!="                  { NE }
  | "<="                  { LE }
  | ">="                  { GE }
  | "<"                   { LT }
  | ">"                   { GT }
  | "in"                  { IN }

  (* Operators - Logical *)
  | "&&"                  { AND }
  | "||"                  { OR }
  | "!"                   { NOT }

  (* Operators - Arithmetic *)
  | "+"                   { PLUS }
  | "-"                   { MINUS }
  | "*"                   { STAR }
  | "/"                   { SLASH }
  | "%"                   { PERCENT }

  (* Symbols *)
  | "("                   { LPAREN }
  | ")"                   { RPAREN }
  | "["                   { LBRACK }
  | "]"                   { RBRACK }
  | "{"                   { LBRACE }
  | "}"                   { RBRACE }
  | "."                   { DOT }
  | ","                   { COMMA }
  | "?"                   { QUESTION }
  | ":"                   { COLON }
  | "=>"                  { ARROW }

  (* Numeric literals *)
  | uint_lit as s         {
      let len = String.length s in
      let num_str = String.sub s 0 (len - 1) in
      try
        if String.length num_str > 2 && num_str.[0] = '0' && num_str.[1] = 'x' then
          UINT (Int64.of_string ("0x" ^ String.sub num_str 2 (String.length num_str - 2)))
        else
          UINT (Int64.of_string num_str)
      with _ ->
        raise (Lexer_error ("Invalid uint literal: " ^ s))
    }

  | float_lit as f        {
      try FLOAT (float_of_string f)
      with _ -> raise (Lexer_error ("Invalid float literal: " ^ f))
    }

  | hex_int as h          {
      try INT (Int64.of_string h)
      with _ -> raise (Lexer_error ("Invalid hex literal: " ^ h))
    }

  | dec_int as i          {
      try INT (Int64.of_string i)
      with _ -> raise (Lexer_error ("Invalid int literal: " ^ i))
    }

  (* String literals - double quotes *)
  | '"' (string_char | "\\\"" | "\\\\")* '"' as s {
      let len = String.length s in
      let content = String.sub s 1 (len - 2) in
      STRING (unescape_string content)
    }

  (* String literals - single quotes *)
  | '\'' (string_char | "\\\'" | "\\\\")* '\'' as s {
      let len = String.length s in
      let content = String.sub s 1 (len - 2) in
      STRING (unescape_string content)
    }

  (* Triple-quoted strings (multiline) *)
  | "\"\"\"" ([^ '"'] | '"' [^ '"'] | "\"\"" [^ '"'])* "\"\"\"" as s {
      let len = String.length s in
      let content = String.sub s 3 (len - 6) in
      STRING content
    }

  (* Bytes literals *)
  | ['b' 'B'] '"' (string_char | "\\\"" | "\\\\")* '"' as s {
      let len = String.length s in
      let content = String.sub s 2 (len - 3) in
      BYTES (Bytes.of_string (unescape_string content))
    }

  (* Raw strings (no escaping) - r"..." or R"..." *)
  | ['r' 'R'] '"' ([^ '"'])* '"' as s {
      let len = String.length s in
      let content = String.sub s 2 (len - 3) in
      STRING content
    }

  (* Identifiers *)
  | letter ident_char* as id { IDENT id }

  (* End of file *)
  | eof                   { EOF }

  (* Unknown character *)
  | _ as c                {
      raise (Lexer_error (Printf.sprintf "Unexpected character: '%c'" c))
    }
