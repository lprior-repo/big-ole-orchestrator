use super::hot_reload::ConfigValidator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    String,
    Number,
    Integer,
    Boolean,
    Object,
    Array,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub field_type: FieldType,
}

pub struct SchemaValidator {
    required: Vec<FieldDef>,
    allow_unknown: bool,
}

impl SchemaValidator {
    pub fn new(required: Vec<FieldDef>, allow_unknown: bool) -> Self {
        Self {
            required,
            allow_unknown,
        }
    }
}

impl ConfigValidator<serde_json::Value> for SchemaValidator {
    fn validate(&self, config: &serde_json::Value) -> Result<(), String> {
        let obj = config
            .as_object()
            .ok_or("config must be a JSON object")?;

        for field in &self.required {
            match obj.get(&field.name) {
                None => return Err(format!("missing required field: {}", field.name)),
                Some(value) => {
                    let ok = match (value, &field.field_type) {
                        (serde_json::Value::String(_), FieldType::String) => true,
                        (serde_json::Value::Number(_), FieldType::Number) => true,
                        (serde_json::Value::Number(n), FieldType::Integer) => {
                            n.is_i64() || n.is_u64()
                        }
                        (serde_json::Value::Bool(_), FieldType::Boolean) => true,
                        (serde_json::Value::Object(_), FieldType::Object) => true,
                        (serde_json::Value::Array(_), FieldType::Array) => true,
                        _ => false,
                    };
                    if !ok {
                        let got = match value {
                            serde_json::Value::Null => "null",
                            serde_json::Value::Bool(_) => "boolean",
                            serde_json::Value::Number(_) => "number",
                            serde_json::Value::String(_) => "string",
                            serde_json::Value::Array(_) => "array",
                            serde_json::Value::Object(_) => "object",
                        };
                        return Err(format!(
                            "{}: field has wrong type: expected {:?}, got {}",
                            field.name, field.field_type, got
                        ));
                    }
                }
            }
        }

        if !self.allow_unknown {
            let known: std::collections::HashSet<&str> =
                self.required.iter().map(|f| f.name.as_str()).collect();
            for key in obj.keys() {
                if !known.contains(key.as_str()) {
                    return Err(format!("unknown field: {}", key));
                }
            }
        }

        Ok(())
    }
}
