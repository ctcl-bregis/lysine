use std::collections::HashMap;
use serde_json::Value;
use crate::errors::Result;

pub mod common;
pub mod colors;

// The global function type definition
pub trait Function: Sync + Send {
    // The global function type definition
    fn call(&self, args: &HashMap<String, Value>) -> Result<Value>;

    // Whether the current function's output should be treated as safe, defaults to `false`
    fn is_safe(&self) -> bool {
        false
    }
}

impl<F> Function for F
where
    F: Fn(&HashMap<String, Value>) -> Result<Value> + Sync + Send,
{
    fn call(&self, args: &HashMap<String, Value>) -> Result<Value> {
        self(args)
    }
}