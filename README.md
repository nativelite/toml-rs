# toml-rs

Minimal TOML 1.0.0 parser — zero dependencies, pure Rust stdlib only.

Part of the [nativelite](https://github.com/nativelite) ecosystem: every crate is auditable in an afternoon, with no third-party dependencies in the critical path.

---

## Features

- **Zero dependencies** — only `std`, no `serde`, no external crates.
- **TOML 1.0.0** compliant for common structures.
- **Strings**: basic (`"…"`), multi-line (`"""…"""`), literal (`'…'`), multi-line literal (`'''…'''`).
- **Numbers**: integers (with `_` separators), floats (including exponential notation).
- **Booleans**: `true` / `false`.
- **Datetimes**: RFC 3339 / ISO 8601 stored as strings.
- **Arrays**: homogeneous and heterogeneous.
- **Tables**: regular `[section]`, dotted keys `a.b.c = v`, inline `{ k = v }`.
- **Comments**: `#` to end of line.
- **Error messages** include line and column numbers.

### Out of scope (v1)
- Array of tables `[[…]]`
- Date arithmetic

---

## API

```rust
use toml_rs::{parse, TomlValue, ParseError};

// Parse a TOML document.
let value: TomlValue = parse(input)?;

// TomlValue variants
match value {
    TomlValue::String(s)   => { /* &str */ }
    TomlValue::Integer(n)  => { /* i64 */ }
    TomlValue::Float(f)    => { /* f64 */ }
    TomlValue::Boolean(b)  => { /* bool */ }
    TomlValue::DateTime(d) => { /* raw RFC 3339 string */ }
    TomlValue::Array(v)    => { /* Vec<TomlValue> */ }
    TomlValue::Table(m)    => { /* HashMap<String, TomlValue> */ }
}

// ParseError carries position info.
if let Err(e) = parse(bad_input) {
    println!("Error at line {}, col {}: {}", e.position.line, e.position.column, e.message);
}
```

---

## Usage example

```rust
use toml_rs::{parse, TomlValue};

let config = r#"
[agent]
name    = "search-agent"
version = 2
enabled = true
model   = "claude-3-opus"

[agent.limits]
max_tokens = 4096
timeout_ms = 30000
"#;

let doc = parse(config).unwrap();
if let TomlValue::Table(root) = &doc {
    if let Some(TomlValue::Table(agent)) = root.get("agent") {
        println!("Agent: {:?}", agent["name"]);
    }
}
```

---

## Running tests

```sh
cargo test
```

---

## License

MIT

