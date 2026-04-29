//! Workflow execution registry for connecting builder to executor.
//!
//! ADR-036: Command identity, correlation, causation.
//!
//! The [`NodeFunctionRegistry`] maps node names to executable functions,
//! enabling runtime dispatch of workflow nodes after build-time construction.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use vo_types::NodeName;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("node not found in registry: {name}")]
    NodeNotFound { name: String },
    #[error("node already registered: {name}")]
    AlreadyRegistered { name: String },
    #[error("type mismatch for node: {name}")]
    TypeMismatch { name: String },
}

pub type BoxedNodeFn<I, O> = Arc<dyn Fn(I) -> O + Send + Sync + 'static>;

struct NodeFnEntry {
    name: String,
    type_id: TypeId,
    func: Box<dyn Any + Send + Sync>,
}

impl Debug for NodeFnEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeFnEntry")
            .field("name", &self.name)
            .field("type_id", &self.type_id)
            .finish()
    }
}

pub struct NodeFunctionRegistry {
    functions: HashMap<NodeName, NodeFnEntry>,
}

impl Debug for NodeFunctionRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeFunctionRegistry")
            .field("node_count", &self.functions.len())
            .finish()
    }
}

impl NodeFunctionRegistry {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }

    pub fn register<I, O>(&mut self, node_name: &str, func: BoxedNodeFn<I, O>) -> Result<(), RegistryError>
    where
        I: 'static,
        O: 'static,
    {
        let name = NodeName::parse(node_name).map_err(|_| RegistryError::AlreadyRegistered {
            name: node_name.to_string(),
        })?;

        if self.functions.contains_key(&name) {
            return Err(RegistryError::AlreadyRegistered {
                name: node_name.to_string(),
            });
        }

        let type_id = TypeId::of::<(I, O)>();
        self.functions.insert(
            name,
            NodeFnEntry {
                name: node_name.to_string(),
                type_id,
                func: Box::new(func),
            },
        );
        Ok(())
    }

    pub fn lookup<I, O>(&self, node_name: &str) -> Result<BoxedNodeFn<I, O>, RegistryError>
    where
        I: 'static,
        O: 'static,
    {
        let name = NodeName::parse(node_name).map_err(|_| RegistryError::NodeNotFound {
            name: node_name.to_string(),
        })?;

        let entry = self.functions.get(&name).ok_or(RegistryError::NodeNotFound {
            name: node_name.to_string(),
        })?;

        let expected_type_id = TypeId::of::<(I, O)>();
        if entry.type_id != expected_type_id {
            return Err(RegistryError::TypeMismatch {
                name: node_name.to_string(),
            });
        }

        entry
            .func
            .downcast_ref::<BoxedNodeFn<I, O>>()
            .cloned()
            .ok_or_else(|| RegistryError::TypeMismatch {
                name: node_name.to_string(),
            })
    }

    pub fn contains(&self, node_name: &str) -> bool {
        NodeName::parse(node_name)
            .ok()
            .map(|name| self.functions.contains_key(&name))
            .unwrap_or(false)
    }

    pub fn len(&self) -> usize {
        self.functions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }
}

impl Default for NodeFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_lookup() {
        let mut registry = NodeFunctionRegistry::new();

        let func: BoxedNodeFn<i32, String> = Arc::new(|input: i32| input.to_string());
        registry
            .register("to-string", func)
            .expect("register should succeed");

        let looked_up: BoxedNodeFn<i32, String> = registry
            .lookup("to-string")
            .expect("lookup should succeed");

        assert_eq!(looked_up(42), "42");
    }

    #[test]
    fn test_lookup_unregistered() {
        let registry = NodeFunctionRegistry::new();
        let result: Result<BoxedNodeFn<i32, String>, _> = registry.lookup("missing");
        assert!(result.is_err());
    }

    #[test]
    fn test_duplicate_registration() {
        let mut registry = NodeFunctionRegistry::new();

        let func: BoxedNodeFn<i32, String> = Arc::new(|input: i32| input.to_string());
        registry
            .register("node", func)
            .expect("first register should succeed");

        let func2: BoxedNodeFn<i32, String> = Arc::new(|input: i32| input.to_string());
        let result = registry.register("node", func2);
        assert!(result.is_err());
    }

    #[test]
    fn test_contains() {
        let mut registry = NodeFunctionRegistry::new();
        assert!(!registry.contains("test"));

        let func: BoxedNodeFn<(), ()> = Arc::new(|_| ());
        registry
            .register("test", func)
            .expect("register should succeed");
        assert!(registry.contains("test"));
    }

    #[test]
    fn test_empty_registry() {
        let registry = NodeFunctionRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_multiple_node_types() {
        let mut registry = NodeFunctionRegistry::new();

        let int_to_str: BoxedNodeFn<i32, String> = Arc::new(|i| i.to_string());
        registry
            .register("int-to-str", int_to_str)
            .expect("register should succeed");

        let str_to_bool: BoxedNodeFn<String, bool> = Arc::new(|s| s.is_empty());
        registry
            .register("str-to-bool", str_to_bool)
            .expect("register should succeed");

        assert_eq!(registry.len(), 2);

        let looked_up_int: BoxedNodeFn<i32, String> = registry
            .lookup("int-to-str")
            .expect("lookup should succeed");
        assert_eq!(looked_up_int(5), "5");

        let looked_up_bool: BoxedNodeFn<String, bool> = registry
            .lookup("str-to-bool")
            .expect("lookup should succeed");
        assert!(!looked_up_bool("hello".to_string()));
        assert!(looked_up_bool("".to_string()));
    }

    #[test]
    fn test_type_mismatch() {
        let mut registry = NodeFunctionRegistry::new();

        let func: BoxedNodeFn<i32, String> = Arc::new(|i| i.to_string());
        registry
            .register("node", func)
            .expect("register should succeed");

        let result: Result<BoxedNodeFn<i32, i32>, _> = registry.lookup("node");
        assert!(matches!(result, Err(RegistryError::TypeMismatch { .. })));
    }
}