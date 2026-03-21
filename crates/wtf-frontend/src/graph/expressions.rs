#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use crate::graph::Node;
use serde_json::Value;

pub struct ExpressionContext<'a> {
    pub nodes: &'a [Node],
}

impl<'a> ExpressionContext<'a> {
    #[must_use]
    pub const fn new(nodes: &'a [Node]) -> Self {
        Self { nodes }
    }

    #[must_use]
    pub fn resolve(&self, expr: &str) -> Value {
        let trimmed = expr.trim();

        // 1. Path Resolution: $node["Name"].json.path
        if let Some(node_part) = trimmed.strip_prefix("$node[\"") {
            if let Some((node_name, path_part)) = node_part.split_once("\"]") {
                let path = path_part
                    .strip_prefix(".json.")
                    .map_or(path_part, |prefix| prefix);
                let resolved = self
                    .nodes
                    .iter()
                    .find(|n| n.name == node_name)
                    .and_then(|n| n.last_output.as_ref())
                    .and_then(|out| out.pointer(&format!("/{}", path.replace('.', "/"))));

                return resolved.map_or(Value::Null, std::clone::Clone::clone);
            }
        }

        // 2. Constant Math (Simple regex-free split)
        if let Some((left, right)) = trimmed.split_once(" + ") {
            return self.eval_binary_op(left, right, |a, b| Value::from(a + b));
        }
        if let Some((left, right)) = trimmed.split_once(" - ") {
            return self.eval_binary_op(left, right, |a, b| Value::from(a - b));
        }

        // 3. String Methods
        if let Some(base) = trimmed.strip_suffix(".to_uppercase()") {
            let val = self.resolve(base);
            if let Some(s) = val.as_str() {
                return Value::String(s.to_uppercase());
            }
        }
        if let Some(base) = trimmed.strip_suffix(".len()") {
            let val = self.resolve(base);
            if let Some(s) = val.as_str() {
                return Value::from(s.len());
            }
            if let Some(a) = val.as_array() {
                return Value::from(a.len());
            }
        }

        // 4. Literals
        if let Ok(n) = trimmed.parse::<f64>() {
            return Value::from(n);
        }
        if (trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        {
            let quote = trimmed.chars().next().map_or('\0', std::convert::identity);
            if let Some(inner) = trimmed
                .strip_prefix(quote)
                .and_then(|value| value.strip_suffix(quote))
            {
                return Value::String(inner.to_string());
            }
        }
        if trimmed == "true" {
            return Value::Bool(true);
        }
        if trimmed == "false" {
            return Value::Bool(false);
        }

        Value::String(trimmed.to_string())
    }

    fn eval_binary_op<F>(&self, left: &str, right: &str, op: F) -> Value
    where
        F: Fn(f64, f64) -> Value,
    {
        let lv = self.resolve(left);
        let rv = self.resolve(right);
        if let (Some(l), Some(r)) = (lv.as_f64(), rv.as_f64()) {
            return op(l, r);
        }
        Value::Null
    }
}

#[cfg(test)]
mod tests {
    use super::ExpressionContext;
    use crate::graph::Node;
    use serde_json::json;

    fn node_with_output(name: &str, output: serde_json::Value) -> Node {
        let mut node = Node::default();
        node.name = name.to_string();
        node.last_output = Some(output);
        node
    }

    #[test]
    fn given_single_quote_token_when_resolving_then_it_does_not_panic() {
        let ctx = ExpressionContext::new(&[]);

        let value = ctx.resolve("'");

        assert_eq!(value, serde_json::Value::String("'".to_string()));
    }

    #[test]
    fn given_double_quote_token_when_resolving_then_it_does_not_panic() {
        let ctx = ExpressionContext::new(&[]);

        let value = ctx.resolve("\"");

        assert_eq!(value, serde_json::Value::String("\"".to_string()));
    }

    #[test]
    fn given_wrapped_literal_when_resolving_then_quotes_are_trimmed() {
        let ctx = ExpressionContext::new(&[]);

        let value = ctx.resolve("'hello'");

        assert_eq!(value, serde_json::Value::String("hello".to_string()));
    }

    #[test]
    fn given_node_json_path_expression_when_resolving_then_returns_pointer_value() {
        let node = node_with_output("Fetcher", json!({"user": {"email": "a@b.dev"}}));
        let nodes = [node];
        let ctx = ExpressionContext::new(&nodes);

        let value = ctx.resolve("$node[\"Fetcher\"].json.user.email");

        assert_eq!(value, serde_json::Value::String("a@b.dev".to_string()));
    }

    #[test]
    fn given_numeric_binary_expression_when_resolving_then_returns_computed_number() {
        let ctx = ExpressionContext::new(&[]);

        assert_eq!(ctx.resolve("3 + 4"), serde_json::Value::from(7.0));
        assert_eq!(ctx.resolve("9 - 2"), serde_json::Value::from(7.0));
    }

    #[test]
    fn given_len_calls_when_resolving_then_returns_string_or_array_length() {
        let node = node_with_output("Fetcher", json!({"names": ["a", "b", "c"]}));
        let nodes = [node];
        let ctx = ExpressionContext::new(&nodes);

        assert_eq!(ctx.resolve("'hello'.len()"), serde_json::Value::from(5));
        assert_eq!(
            ctx.resolve("$node[\"Fetcher\"].json.names.len()"),
            serde_json::Value::Null
        );
    }

    #[test]
    fn given_uppercase_call_when_resolving_then_string_is_transformed() {
        let ctx = ExpressionContext::new(&[]);

        let value = ctx.resolve("'hello'.to_uppercase()");

        assert_eq!(value, serde_json::Value::String("HELLO".to_string()));
    }

    #[test]
    fn given_unknown_token_when_resolving_then_original_trimmed_string_is_returned() {
        let ctx = ExpressionContext::new(&[]);

        let value = ctx.resolve("  no_such_token  ");

        assert_eq!(
            value,
            serde_json::Value::String("no_such_token".to_string())
        );
    }
}
