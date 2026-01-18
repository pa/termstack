use serde_json::Value;
use serde_json_path::JsonPath;

use crate::error::{Result, TermStackError};

/// JSONPath extractor for filtering and selecting data
#[derive(Debug, Clone)]
pub struct JsonPathExtractor {
    path: JsonPath,
    path_str: String,
}

impl JsonPathExtractor {
    pub fn new(path: &str) -> Result<Self> {
        // Special case for "@this" which means return the whole object
        if path == "@this" {
            return Ok(Self {
                path: JsonPath::parse("$").map_err(|e| {
                    TermStackError::DataProvider(format!("Failed to parse JSONPath: {}", e))
                })?,
                path_str: path.to_string(),
            });
        }

        let parsed_path = JsonPath::parse(path).map_err(|e| {
            TermStackError::DataProvider(format!("Failed to parse JSONPath '{}': {}", path, e))
        })?;

        Ok(Self {
            path: parsed_path,
            path_str: path.to_string(),
        })
    }

    /// Extract array of values from data
    pub fn extract(&self, data: &Value) -> Result<Vec<Value>> {
        // Special handling for @this
        if self.path_str == "@this" {
            return match data {
                Value::Array(arr) => Ok(arr.clone()),
                other => Ok(vec![other.clone()]),
            };
        }

        let nodes = self.path.query(data);
        let values: Vec<Value> = nodes.all().into_iter().map(|v| v.clone()).collect();

        if values.is_empty() {
            // If no results, return empty array
            Ok(Vec::new())
        } else {
            Ok(values)
        }
    }

    /// Extract single value from data
    pub fn extract_single(&self, data: &Value) -> Result<Option<Value>> {
        if self.path_str == "@this" {
            return Ok(Some(data.clone()));
        }

        let nodes = self.path.query(data);
        Ok(nodes.all().first().map(|v| (*v).clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_array() {
        let data = json!({
            "items": [
                {"name": "item1"},
                {"name": "item2"}
            ]
        });

        let extractor = JsonPathExtractor::new("$.items[*]").unwrap();
        let result = extractor.extract(&data).unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["name"], "item1");
        assert_eq!(result[1]["name"], "item2");
    }

    #[test]
    fn test_extract_single() {
        let data = json!({
            "metadata": {
                "name": "test-name"
            }
        });

        let extractor = JsonPathExtractor::new("$.metadata.name").unwrap();
        let result = extractor.extract_single(&data).unwrap();

        assert!(result.is_some());
        assert_eq!(result.unwrap(), "test-name");
    }

    #[test]
    fn test_extract_at_this() {
        let data = json!({
            "name": "test"
        });

        let extractor = JsonPathExtractor::new("@this").unwrap();
        let result = extractor.extract(&data).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "test");
    }
}
