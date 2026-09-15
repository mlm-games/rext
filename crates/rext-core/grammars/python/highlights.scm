(comment) @comment
(string) @string
(integer) @number
(float) @number
[(true) (false) (none)] @number
[
  "import" "from" "as" "def" "class" "return" "if" "elif" "else"
  "for" "while" "in" "not" "and" "or" "is" "lambda" "with"
  "try" "except" "finally" "raise" "assert" "del" "global"
  "nonlocal" "pass" "break" "continue" "yield" "await" "async"
] @keyword
(identifier) @variable
(attribute attribute: (identifier) @property)
(call function: (identifier) @function)
(call function: (attribute attribute: (identifier) @function))
(class_definition name: (identifier) @type)
(function_definition name: (identifier) @function)
(decorated_definition definition: (function_definition name: (identifier) @function))
(decorated_definition definition: (class_definition name: (identifier) @type))
(type (identifier) @type)
[
  "=" "+=" "-=" "*=" "/=" "+" "-" "*" "/" "%" "**" "//"
  "==" "!=" "<" ">" "<=" ">=" "->" ":" ";" "," "."
] @operator
[
  "(" ")" "[" "]" "{" "}"
] @punctuation
