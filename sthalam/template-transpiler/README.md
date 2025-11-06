# huml-ml

OCaml parser for [HUML](https://huml.io) (Human Markup Language).

## Playground

This parser has been compiled with [`Js_of_ocaml`](https://github.com/ocsigen/Js_of_ocaml)
and deployed here: [kaustubh.page/huml](https://kaustubh.page/huml).

## CLI Usage

Install the `huml-cli` package to use the command-line interface.

```bash
# Parse HUML file and output JSON to stdout
huml input.huml

# Parse HUML file and save JSON to file
huml input.huml -o output.json
huml input.huml --output output.json

# Show help
huml -h
huml --help

# Show version
huml -v
huml --version
```

## Library Usage

Install the `huml` package to use just the library (without the CLI).

```ocaml
open Huml

let parse_huml_string content =
  let lexbuf = Lexing.from_string content in
  match parse lexbuf with
  | Ok ast -> ast
  | Error msg -> failwith msg

(* Example *)
let huml_content = {|
name: "John Doe"
age: 30
active: true
|} in
let ast = parse_huml_string huml_content in
(* ast is of type Types.Ast.t, compatible with Yojson.Safe.t *)
```

The parsed AST is compatible with [`Yojson.Safe.t`](https://ocaml-doc.github.io/odoc-examples/yojson/Yojson/Safe/index.html).

## JavaScript Usage

First build the `huml_js.bc.js` target using `dune`:

```bash
dune build bin/huml_js.bc.js
```

Then use the generated JavaScript file from `_build/default/bin/huml_js.bc.js`. This
file can be used as a browser `<script src="...">` or in another runtime like Node.js.

It exports a `huml` object with a single `parse` method that takes a string and returns
an object of type `{ "error": string, "output": string }`. If `"error"` is an empty
string, then the parsing was successful and `"output"` contains the JSON string.

```javascript
const result = huml.parse(...);

if (result.error) {
    console.error("Error parsing HUML:", result.error);
} else {
    console.log("Parsed JSON:", result.output);
}
```

Look at [`examples/playground.html`](examples/playground.html) for a working example.

## Installation

Setup `opam`, `dune`, clone this repository, and run:

```bash
# Just the library
dune build -p huml @install

# CLI
dune build -p huml-cli @install
```

## License

MIT.
