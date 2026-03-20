# Parsing Rules

This document describes how `ccval` parses commit message structure.

See [`README.md`](README.md) for the distinction between parseable messages and messages that pass validation, and [`VALIDATION.md`](VALIDATION.md) for rule-based validation.

## Overall Message Format

In the examples below, `\n` means a newline character in the actual commit message.

```text
type[(scope)][!]: description\n
[blank line, then body]
[blank line, then footers]
```

Header rules:

- The header must contain a commit type.
- The type must start with an alphanumeric character or underscore.
- `scope` is optional.
- If present, the scope must start with an alphanumeric character or underscore.
- Type and scope may then contain alphanumeric characters, underscores, and dashes, but must not end with a dash.
- `!` is optional and marks a breaking change.
- `: ` after the type or scope is required.
- The description must not be empty.
- The actual commit message text must end the header with a newline character.

## Message Structure

After the header:

- A body or footer must be separated from the header by a blank line.
- If footers follow a body, they must also be separated from the body by a blank line.
- A body must end with a newline character.
- A footer value must end with a newline character.
- CRLF line endings are normalized to LF before parsing.
- Control characters other than newline are rejected.

## Footer Forms

Footers are recognized in these forms:

- `Token: value`
- `Token #value`
- `BREAKING CHANGE: value`
- `BREAKING CHANGE #value`

A footer is recognized only after the blank-line separator. Footer-like lines in the body remain part of the body.

## Parse Errors

A **parse error** means the commit message structure is malformed. Validation happens after parsing succeeds; see [`VALIDATION.md`](VALIDATION.md) for rule failures.

Examples:

- missing the final newline in the header
- missing `: ` after the type or scope
- missing description
- missing blank line before body or footers

## Examples

Parseable header-only commit:

```text
feat: add CLI flag\n
```

Parseable message with body:

```text
feat(api): add repository option\n
\n
Allow validating commits from another repository.\n
```

Parseable message with footers:

```text
fix!: change footer parsing\n
\n
Handle footer separators consistently.\n
\n
Closes #123\n
BREAKING CHANGE: footer parsing is stricter now\n
```

Not parseable: missing description

```text
feat: \n
```

Not parseable: missing blank line before body

```text
feat: add CLI flag\n
body starts immediately\n
```

Parseable but may fail validation:

```text
feat:  add CLI flag \n
```

For example, presets may reject this description because of leading or trailing spaces. See [`VALIDATION.md`](VALIDATION.md) for validation rules.
