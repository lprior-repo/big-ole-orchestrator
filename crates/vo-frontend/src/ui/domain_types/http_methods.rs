use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HttpMethod {
    Get,
    #[default]
    Post,
    Put,
    Delete,
    Patch,
}

impl HttpMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
            Self::Patch => "PATCH",
        }
    }

    pub fn from_str_ignore_case(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "DELETE" => Self::Delete,
            "PATCH" => Self::Patch,
            _ => Self::Post,
        }
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Get, Self::Post, Self::Put, Self::Delete, Self::Patch]
    }

    #[must_use]
    pub fn parse(s: &str) -> Self {
        Self::from_str_ignore_case(s)
    }
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for HttpMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "DELETE" => Ok(Self::Delete),
            "PATCH" => Ok(Self::Patch),
            _ => Err(format!("Invalid HTTP method: {s}")),
        }
    }
}
