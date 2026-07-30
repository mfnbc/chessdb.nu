use nu_protocol::{LabeledError, PipelineData, Record, Span, Value};

/// Recursively convert a `serde_json::Value` into a `nu_protocol::Value`.
pub fn json_to_nu_value(val: serde_json::Value, span: nu_protocol::Span) -> Value {
    match val {
        serde_json::Value::Null => Value::nothing(span),
        serde_json::Value::Bool(b) => Value::bool(b, span),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::int(i, span)
            } else if let Some(f) = n.as_f64() {
                Value::float(f, span)
            } else {
                Value::string(n.to_string(), span)
            }
        }
        serde_json::Value::String(s) => Value::string(s, span),
        serde_json::Value::Array(arr) => {
            let items: Vec<Value> = arr.into_iter().map(|v| json_to_nu_value(v, span)).collect();
            Value::list(items, span)
        }
        serde_json::Value::Object(obj) => {
            let mut rec = Record::new();
            for (k, v) in obj {
                rec.push(k, json_to_nu_value(v, span));
            }
            Value::record(rec, span)
        }
    }
}

/// Shared by every plugin command whose input is "a string, or a list of
/// strings, each mapped through the same conversion" (`canonicalize-fen`,
/// `zobrist`, `pgn-to-batch`): applies `f` to a single string, or to each
/// element of a list, producing one output value per input value.
pub fn map_string_or_list<F>(input: PipelineData, span: Span, f: F) -> Result<PipelineData, LabeledError>
where
    F: Fn(&str, Span) -> Result<Value, LabeledError>,
{
    match input.into_value(span)? {
        Value::String { val, .. } => Ok(PipelineData::Value(f(&val, span)?, None)),
        Value::List { vals, .. } => {
            let mut results = Vec::with_capacity(vals.len());
            for v in vals {
                results.push(f(v.as_str()?, span)?);
            }
            Ok(PipelineData::Value(Value::list(results, span), None))
        }
        _ => Err(LabeledError::new("Expected string or list of strings")
            .with_label("invalid input type", span)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upper(s: &str, span: Span) -> Result<Value, LabeledError> {
        Ok(Value::string(s.to_uppercase(), span))
    }

    #[test]
    fn single_string_maps_to_single_value() {
        let span = Span::test_data();
        let input = PipelineData::Value(Value::string("abc", span), None);
        let out = map_string_or_list(input, span, upper).unwrap().into_value(span).unwrap();
        assert_eq!(out.as_str().unwrap(), "ABC");
    }

    #[test]
    fn list_maps_each_element_preserving_order() {
        let span = Span::test_data();
        let input = PipelineData::Value(
            Value::list(vec![Value::string("a", span), Value::string("b", span)], span),
            None,
        );
        let out = map_string_or_list(input, span, upper).unwrap().into_value(span).unwrap();
        let items = out.as_list().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_str().unwrap(), "A");
        assert_eq!(items[1].as_str().unwrap(), "B");
    }

    #[test]
    fn non_string_non_list_input_errors() {
        let span = Span::test_data();
        let input = PipelineData::Value(Value::int(42, span), None);
        assert!(map_string_or_list(input, span, upper).is_err());
    }

    #[test]
    fn closure_error_propagates() {
        let span = Span::test_data();
        let input = PipelineData::Value(Value::string("x", span), None);
        let result = map_string_or_list(input, span, |_, span| {
            Err(LabeledError::new("boom").with_label("test", span))
        });
        assert!(result.is_err());
    }
}
