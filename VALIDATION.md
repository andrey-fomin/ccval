# Validation Rules

After a commit message is parsed, `ccval` applies validation rules from your configuration.

See [`PARSING.md`](PARSING.md) for the commit message grammar and parse errors.

## Available Fields

You can attach rules to these fields:

```yaml
preset: default

message: <RULES>
header: <RULES>
type: <RULES>
scope: <RULES>
description: <RULES>
body: <RULES>
footer-token: <RULES>
footer-value: <RULES>
footers:
  Closes: <RULES>
```

- `message` is the full commit message
- `header` is the first line, including its trailing newline
- `type`, `scope`, and `description` come from the parsed header
- `body` is everything between the blank line after the header and the first footer, including its trailing newline when present
- `footer-token` and `footer-value` apply to every footer
- `footer-value` includes its trailing newline
- `footers.<name>` applies to a specific footer value such as `Closes`

## Available Rules

| Rule | Meaning |
|------|---------|
| `max-length` | Maximum total length, including newlines |
| `max-line-length` | Maximum per-line length, excluding newline characters |
| `required` | Field must be present |
| `forbidden` | Field must not be present |
| `regexes` | All regexes must match |
| `values` | Field must match one of the allowed values |

## Notes About Length Rules

- `max-length` checks the full field length exactly as stored
- `max-line-length` checks each line separately
- for fields that include a trailing newline, `max-line-length` ignores that trailing newline when measuring line length

This means `max-length` and `max-line-length` are not interchangeable.

## Rule Examples

Allow only selected commit types:

```yaml
type:
  values:
    - feat
    - fix
    - docs
```

Require a scope:

```yaml
scope:
  required: true
```

Reject descriptions with leading or trailing spaces:

```yaml
description:
  regexes:
    - '^[^ ].*'
    - '^.*[^ ]$'
```

Limit header line length:

```yaml
header:
  max-line-length: 50
```

Require a specific footer:

```yaml
footers:
  Closes:
    required: true
```

## Presets

`ccval` includes these presets:

- `default` - formatting rules for description spacing and newline handling in body/footer values
- `strict` - `default` plus header length limits and common type/scope restrictions

Start with a preset and override only the rules you need.

## Validation Errors

Validation errors are reported after parsing succeeds.

Examples:

- `type 'foo' is not in allowed values: ["feat", "fix"]`
- `header line length 51 is greater than 50`
- `required scope is missing`
- `description does not match regex '^.*[^ ]$'`

## Example: Parseable but Fails Validation

As in [`PARSING.md`](PARSING.md), `\n` denotes a newline character in the example below.

```yaml
preset: strict

scope:
  required: true

footers:
  Closes:
    required: true
```

This message is parseable, but it fails validation because the required `scope` and `Closes` footer are missing:

```text
feat: add repository option\n
```
