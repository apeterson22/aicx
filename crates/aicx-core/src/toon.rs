use serde::Serialize;
use serde_json::Value;

use crate::error::Result;

pub fn render_toon<T: Serialize>(value: &T) -> Result<String> {
    let value = serde_json::to_value(value)?;
    let mut output = String::new();
    render_value(&value, 0, &mut output);
    if output.ends_with('\n') {
        output.pop();
    }
    Ok(output)
}

fn render_value(value: &Value, indent: usize, output: &mut String) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(flag) => output.push_str(if *flag { "true" } else { "false" }),
        Value::Number(number) => output.push_str(&number.to_string()),
        Value::String(text) => {
            if let Ok(encoded) = serde_json::to_string(text) {
                output.push_str(&encoded);
            } else {
                output.push_str("\"\"");
            }
        }
        Value::Array(items) => render_array(items, indent, output),
        Value::Object(fields) => render_object(fields, indent, output),
    }
}

fn render_indent(indent: usize, output: &mut String) {
    for _ in 0..indent {
        output.push(' ');
    }
}

fn render_key(key: &str, output: &mut String) {
    if key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/'))
    {
        output.push_str(key);
        return;
    }

    if let Ok(encoded_key) = serde_json::to_string(key) {
        output.push_str(&encoded_key);
    } else {
        output.push_str("\"\"");
    }
}

fn render_array(items: &[Value], indent: usize, output: &mut String) {
    if items.is_empty() {
        output.push_str("[]");
        return;
    }
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        render_indent(indent, output);
        output.push_str("- ");
        match item {
            Value::Array(_) | Value::Object(_) => {
                output.push('\n');
                render_value(item, indent + 2, output);
            }
            _ => render_value(item, indent + 2, output),
        }
    }
}

fn render_object(fields: &serde_json::Map<String, Value>, indent: usize, output: &mut String) {
    if fields.is_empty() {
        output.push_str("{}");
        return;
    }
    for (index, (key, value)) in fields.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        render_indent(indent, output);
        render_key(key, output);
        output.push(':');
        match value {
            Value::Array(_) | Value::Object(_) => {
                output.push('\n');
                render_value(value, indent + 2, output);
            }
            _ => {
                output.push(' ');
                render_value(value, indent + 2, output);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::render_toon;
    use serde::Serialize;
    use std::collections::BTreeMap;

    #[derive(Serialize)]
    struct Example {
        title: String,
        values: Vec<u32>,
    }

    #[test]
    fn renders_nested_values_without_json_braces() {
        let rendered = render_toon(&Example {
            title: "demo".to_string(),
            values: vec![1, 2],
        })
        .expect("render toon");

        assert!(rendered.contains("title:"));
        assert!(rendered.contains("values:"));
        assert!(!rendered.contains('{'));
        assert!(!rendered.contains('}'));
    }

    #[test]
    fn quotes_object_keys_that_need_escaping() {
        let mut value = BTreeMap::new();
        value.insert("needs space".to_string(), "demo".to_string());

        let rendered = render_toon(&value).expect("render toon");

        assert!(rendered.contains("\"needs space\":"));
    }
}
