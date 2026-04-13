use crate::search::error::SearchError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub terms: Vec<String>,
}

impl Query {
    pub fn new(terms: Vec<String>) -> Self {
        Self { terms }
    }
}

pub struct QueryParser;

impl QueryParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, query_str: &str) -> Result<Query, SearchError> {
        let trimmed = query_str.trim();
        if trimmed.is_empty() {
            return Err(SearchError::EmptyQuery);
        }
        let terms: Vec<String> = trimmed
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        if terms.is_empty() {
            return Err(SearchError::EmptyQuery);
        }
        Ok(Query::new(terms))
    }
}

impl Default for QueryParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qp_001_parse_single_term() {
        let parser = QueryParser::new();
        let query = parser.parse("hello").unwrap();
        assert_eq!(query.terms.len(), 1);
        assert_eq!(query.terms[0], "hello");
    }

    #[test]
    fn qp_002_parse_multiple_terms() {
        let parser = QueryParser::new();
        let query = parser.parse("hello world").unwrap();
        assert_eq!(query.terms.len(), 2);
        assert_eq!(query.terms[0], "hello");
        assert_eq!(query.terms[1], "world");
    }

    #[test]
    fn qp_003_parse_lowercase() {
        let parser = QueryParser::new();
        let query = parser.parse("Hello WORLD").unwrap();
        assert_eq!(query.terms[0], "hello");
        assert_eq!(query.terms[1], "world");
    }

    #[test]
    fn qp_004_parse_empty_string() {
        let parser = QueryParser::new();
        let result = parser.parse("");
        assert!(result.is_err());
    }

    #[test]
    fn qp_005_parse_whitespace_only() {
        let parser = QueryParser::new();
        let result = parser.parse("   ");
        assert!(result.is_err());
    }

    #[test]
    fn qp_006_parse_with_special_chars() {
        let parser = QueryParser::new();
        let query = parser.parse("hello-world_test").unwrap();
        assert_eq!(query.terms.len(), 3);
    }
}
