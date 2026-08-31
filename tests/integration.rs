use toml_rs::{parse, TomlValue};
use std::collections::HashMap;

// ─── Helper ──────────────────────────────────────────────────────────────────

fn table(pairs: &[(&str, TomlValue)]) -> TomlValue {
    let mut m = HashMap::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v.clone());
    }
    TomlValue::Table(m)
}

// ─── Basic scalars ───────────────────────────────────────────────────────────

#[test]
fn test_string_basic() {
    let v = parse(r#"key = "hello""#).unwrap();
    assert_eq!(v, table(&[("key", TomlValue::String("hello".into()))]));
}

#[test]
fn test_string_escape_sequences() {
    let v = parse(r#"key = "tab:\there""#).unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["key"], TomlValue::String("tab:\there".into()));
    }
}

#[test]
fn test_string_unicode_escape() {
    let v = parse(r#"key = "\u0041""#).unwrap(); // 'A'
    if let TomlValue::Table(t) = v {
        assert_eq!(t["key"], TomlValue::String("A".into()));
    }
}

#[test]
fn test_literal_string() {
    let v = parse(r#"key = 'C:\Users\nobody'"#).unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["key"], TomlValue::String(r"C:\Users\nobody".into()));
    }
}

#[test]
fn test_ml_basic_string() {
    let input = "key = \"\"\"\nline1\nline2\n\"\"\"";
    let v = parse(input).unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["key"], TomlValue::String("line1\nline2\n".into()));
    }
}

#[test]
fn test_ml_literal_string() {
    let input = "key = '''\nhello\nworld\n'''";
    let v = parse(input).unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["key"], TomlValue::String("hello\nworld\n".into()));
    }
}

#[test]
fn test_integer() {
    let v = parse("x = 42").unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["x"], TomlValue::Integer(42));
    }
}

#[test]
fn test_negative_integer() {
    let v = parse("x = -7").unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["x"], TomlValue::Integer(-7));
    }
}

#[test]
fn test_integer_with_underscores() {
    let v = parse("x = 1_000_000").unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["x"], TomlValue::Integer(1000000));
    }
}

#[test]
fn test_float() {
    let v = parse("x = 3.14").unwrap();
    if let TomlValue::Table(t) = v {
        if let TomlValue::Float(f) = t["x"] {
            assert!((f - 3.14).abs() < 1e-10);
        } else {
            panic!("expected float");
        }
    }
}

#[test]
fn test_float_exponential() {
    let v = parse("x = 6.022e23").unwrap();
    if let TomlValue::Table(t) = v {
        if let TomlValue::Float(f) = t["x"] {
            assert!((f - 6.022e23).abs() < 1e15);
        } else {
            panic!("expected float");
        }
    }
}

#[test]
fn test_float_negative_exp() {
    let v = parse("x = 1.0e-3").unwrap();
    if let TomlValue::Table(t) = v {
        if let TomlValue::Float(f) = t["x"] {
            assert!((f - 0.001).abs() < 1e-12);
        } else {
            panic!("expected float");
        }
    }
}

#[test]
fn test_boolean_true() {
    let v = parse("flag = true").unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["flag"], TomlValue::Boolean(true));
    }
}

#[test]
fn test_boolean_false() {
    let v = parse("flag = false").unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["flag"], TomlValue::Boolean(false));
    }
}

#[test]
fn test_datetime() {
    let v = parse("dt = 2024-01-15T12:30:00Z").unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["dt"], TomlValue::DateTime("2024-01-15T12:30:00Z".into()));
    }
}

// ─── Arrays ──────────────────────────────────────────────────────────────────

#[test]
fn test_array_ints() {
    let v = parse("arr = [1, 2, 3]").unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["arr"], TomlValue::Array(vec![
            TomlValue::Integer(1),
            TomlValue::Integer(2),
            TomlValue::Integer(3),
        ]));
    }
}

#[test]
fn test_array_strings() {
    let v = parse(r#"arr = ["a", "b"]"#).unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["arr"], TomlValue::Array(vec![
            TomlValue::String("a".into()),
            TomlValue::String("b".into()),
        ]));
    }
}

#[test]
fn test_array_multiline() {
    let input = "arr = [\n  1,\n  2,\n  3,\n]";
    let v = parse(input).unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["arr"], TomlValue::Array(vec![
            TomlValue::Integer(1),
            TomlValue::Integer(2),
            TomlValue::Integer(3),
        ]));
    }
}

#[test]
fn test_array_empty() {
    let v = parse("arr = []").unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["arr"], TomlValue::Array(vec![]));
    }
}

#[test]
fn test_array_mixed() {
    let v = parse(r#"arr = [1, "two", true]"#).unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["arr"], TomlValue::Array(vec![
            TomlValue::Integer(1),
            TomlValue::String("two".into()),
            TomlValue::Boolean(true),
        ]));
    }
}

// ─── Tables ──────────────────────────────────────────────────────────────────

#[test]
fn test_table_header() {
    let input = "[server]\nhost = \"localhost\"\nport = 8080";
    let v = parse(input).unwrap();
    if let TomlValue::Table(root) = v {
        if let TomlValue::Table(server) = &root["server"] {
            assert_eq!(server["host"], TomlValue::String("localhost".into()));
            assert_eq!(server["port"], TomlValue::Integer(8080));
        } else {
            panic!("server not a table");
        }
    }
}

#[test]
fn test_dotted_table() {
    let input = "[database.connection]\nurl = \"postgres://localhost/db\"";
    let v = parse(input).unwrap();
    if let TomlValue::Table(root) = v {
        if let TomlValue::Table(db) = &root["database"] {
            if let TomlValue::Table(conn) = &db["connection"] {
                assert_eq!(conn["url"], TomlValue::String("postgres://localhost/db".into()));
            } else { panic!("connection not a table"); }
        } else { panic!("database not a table"); }
    }
}

#[test]
fn test_inline_table() {
    let input = r#"point = { x = 1, y = 2 }"#;
    let v = parse(input).unwrap();
    if let TomlValue::Table(root) = v {
        if let TomlValue::Table(pt) = &root["point"] {
            assert_eq!(pt["x"], TomlValue::Integer(1));
            assert_eq!(pt["y"], TomlValue::Integer(2));
        } else { panic!("expected table"); }
    }
}

#[test]
fn test_inline_table_empty() {
    let v = parse("pt = {}").unwrap();
    if let TomlValue::Table(root) = v {
        assert_eq!(root["pt"], TomlValue::Table(HashMap::new()));
    }
}

// ─── Comments ────────────────────────────────────────────────────────────────

#[test]
fn test_comments_ignored() {
    let input = "# top comment\nkey = 1 # inline comment\n";
    let v = parse(input).unwrap();
    if let TomlValue::Table(t) = v {
        assert_eq!(t["key"], TomlValue::Integer(1));
    }
}

// ─── Dotted keys ─────────────────────────────────────────────────────────────

#[test]
fn test_dotted_keys() {
    let input = "a.b.c = 42";
    let v = parse(input).unwrap();
    if let TomlValue::Table(root) = v {
        if let TomlValue::Table(a) = &root["a"] {
            if let TomlValue::Table(b) = &a["b"] {
                assert_eq!(b["c"], TomlValue::Integer(42));
            } else { panic!(); }
        } else { panic!(); }
    }
}

// ─── Error handling ──────────────────────────────────────────────────────────

#[test]
fn test_error_duplicate_key() {
    let input = "key = 1\nkey = 2";
    assert!(parse(input).is_err());
}

#[test]
fn test_error_unterminated_string() {
    assert!(parse(r#"key = "unterminated"#).is_err());
}

#[test]
fn test_error_invalid_escape() {
    assert!(parse(r#"key = "\q""#).is_err());
}

#[test]
fn test_error_has_position() {
    let err = parse("key = @").unwrap_err();
    assert!(err.position.line >= 1);
    assert!(err.position.column >= 1);
}

// ─── Real-world examples ─────────────────────────────────────────────────────

#[test]
fn test_agent_config() {
    let input = r#"
# Agent configuration
[agent]
name = "search-agent"
version = 2
enabled = true
model = "claude-3-opus"
temperature = 0.7

[agent.limits]
max_tokens = 4096
timeout_ms = 30000

[credentials]
vault_path = '/run/secrets/agent'
rotate_on_start = false
"#;
    let v = parse(input).unwrap();
    if let TomlValue::Table(root) = v {
        if let TomlValue::Table(agent) = &root["agent"] {
            assert_eq!(agent["name"], TomlValue::String("search-agent".into()));
            assert_eq!(agent["version"], TomlValue::Integer(2));
            assert_eq!(agent["enabled"], TomlValue::Boolean(true));
            if let TomlValue::Float(t) = agent["temperature"] {
                assert!((t - 0.7).abs() < 1e-10);
            }
            if let TomlValue::Table(limits) = &agent["limits"] {
                assert_eq!(limits["max_tokens"], TomlValue::Integer(4096));
            }
        } else { panic!("expected agent table"); }
        if let TomlValue::Table(creds) = &root["credentials"] {
            assert_eq!(creds["rotate_on_start"], TomlValue::Boolean(false));
        }
    }
}

#[test]
fn test_cargo_toml_like() {
    let input = r#"
[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
description = "A sample crate"

[dependencies]

[profile.release]
opt-level = 3
debug = false
"#;
    let v = parse(input).unwrap();
    if let TomlValue::Table(root) = v {
        if let TomlValue::Table(pkg) = &root["package"] {
            assert_eq!(pkg["name"], TomlValue::String("my-crate".into()));
            assert_eq!(pkg["edition"], TomlValue::String("2021".into()));
        }
        if let TomlValue::Table(profile) = &root["profile"] {
            if let TomlValue::Table(release) = &profile["release"] {
                assert_eq!(release["opt-level"], TomlValue::Integer(3));
                assert_eq!(release["debug"], TomlValue::Boolean(false));
            }
        }
    }
}

#[test]
fn test_nested_arrays_of_strings() {
    let input = r#"tags = ["rust", "parser", "toml", "zero-dep"]"#;
    let v = parse(input).unwrap();
    if let TomlValue::Table(t) = v {
        if let TomlValue::Array(tags) = &t["tags"] {
            assert_eq!(tags.len(), 4);
            assert_eq!(tags[0], TomlValue::String("rust".into()));
        }
    }
}
