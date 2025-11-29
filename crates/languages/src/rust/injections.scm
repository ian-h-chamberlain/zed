([
    (line_comment)
    (block_comment)
] @injection.content
    (#set! injection.language "comment"))

(macro_invocation
    macro: [
        ((identifier) @_macro_name)
        (scoped_identifier (identifier) @_macro_name .)
    ]
    (token_tree) @injection.content
    (#set! injection.language "rust"))

; we need a better way for the leptos extension to declare that
; it wants to inject inside of rust, instead of modifying the rust
; injections to support leptos injections
(macro_invocation
    macro: [
        ((identifier) @_macro_name)
        (scoped_identifier (identifier) @_macro_name .)
    ]
    (#any-of? @_macro_name "view" "html")
    (token_tree) @injection.content
    (#set! injection.language "rstml")
    )

(macro_invocation
    macro: [
        ((identifier) @_macro_name)
        (scoped_identifier (identifier) @_macro_name .)
    ]
    (#any-of? @_macro_name "sql")
    (_) @injection.content
    (#set! injection.language "sql")
    )

; lazy_regex
(macro_invocation
    macro: [
        ((identifier) @_macro_name)
        (scoped_identifier (identifier) @_macro_name .)
    ]
    (token_tree [
        (string_literal (string_content) @injection.content)
        (raw_string_literal (string_content) @injection.content)
    ])
    (#set! injection.language "regex")
    (#any-of? @_macro_name "regex" "bytes_regex")
)

(call_expression
    function: (scoped_identifier) @_fn_path
    arguments: (arguments
        [
            (string_literal (string_content) @injection.content)
            (raw_string_literal (string_content) @injection.content)
        ]
    )

    (#match? @_fn_path ".*Regex(Builder)?::new")
    (#set! injection.language "regex")
)

; format_args macros
(macro_invocation
    macro: [
        ((identifier) @_macro_name)
        (scoped_identifier (identifier) @_macro_name .)
    ]
    (token_tree . [
        (string_literal (string_content) @injection.content)
        (raw_string_literal (string_content) @injection.content)
    ])

    (#any-of? @_macro_name
        ; std
        "print" "println" "eprint" "eprintln" "format" "format_args" "todo" "panic"
        "unreachable" "unimplemented" "compile_error"
        ; asm is really a subset of the full format string syntax but close enough
        "asm" "global_asm" "naked_asm"
        ; log
        "crit" "trace" "debug" "info" "warn" "error"
        ; anyhow
        "anyhow" "bail"
        ; syn
        "format_ident"
    )
    (#set! injection.language "rust-format-args")
)

(macro_invocation
    macro: [
        ((identifier) @_macro_name)
        (scoped_identifier (identifier) @_macro_name .)
    ]
    (token_tree . (_) . [
        (string_literal (string_content) @injection.content)
        (raw_string_literal (string_content) @injection.content)
    ])
    ; std
    (#any-of? @_macro_name "write" "writeln" "assert" "debug_assert")
    (#set! injection.language "rust-format-args")
)

(macro_invocation
    macro: [
        ((identifier) @_macro_name)
        (scoped_identifier (identifier) @_macro_name .)
    ]
    (token_tree . (_) . (_) . [
        (string_literal (string_content) @injection.content)
        (raw_string_literal (string_content) @injection.content)
    ])
    ; std
    (#any-of? @_macro_name "assert_eq" "assert_ne")
    (#set! injection.language "rust-format-args")
)
