; regular escapes like `\n` are detected using another grammar
; Here, we only detect `{{` and `}}` as escapes for `{` and `}`
(escaped) @string.escape

[
    "#"
    "?"
    (type)
] @punctuation.special.format

[
    (sign)
    (fill)
    (align)
] @operator.format

[
    (width)
    (precision)
] @number.format

(colon) @punctuation.format

(identifier) @variable

; SCREAMING_CASE is assumed to be constant
((identifier) @constant
    (#match? @constant "^[A-Z_]+$"))

[
    "{"
    "}"
] @punctuation.delimiter.format
