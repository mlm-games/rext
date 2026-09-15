(line_comment) @comment
(block_comment) @comment
(string_literal) @string
(raw_string_literal) @string
(char_literal) @string
(boolean_literal) @number
(integer_literal) @number
(float_literal) @number
[
  "fn" "let" "const" "static" "struct" "enum" "impl" "trait"
  "pub" "use" "mod"
  "if" "else" "match" "for" "while" "loop" "in" "return"
  "break" "continue" "where" "async" "await" "move" "ref"
  "type" "union" "unsafe" "extern" "dyn" "as" "try" "default" "gen" "yield"
] @keyword
(type_identifier) @type
(primitive_type) @type.builtin
(self) @variable.special
(field_identifier) @property
(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (field_identifier) @function))
(function_item name: (identifier) @function)
(macro_invocation macro: (identifier) @function)
(lifetime) @type
[
  "=" "+" "-" "*" "/" "%" "!" "==" "!=" "<" ">" "<=" ">="
  "&&" "||" "&" "|" "^" "<<" ">>" "+=" "-=" "*=" "/=" "%="
  "->" "=>" "::" ":" ";" "," "." ".." "..=" "@" "_" "?"
] @operator
[
  "(" ")" "[" "]" "{" "}"
] @punctuation
