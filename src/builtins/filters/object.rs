// Filters operating on numbers
use std::collections::HashMap;
use serde_json::value::Value;
use crate::errors::{Error, Result};

// Returns a value by a `key` argument from a given object
pub fn get(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let default = args.get("default");
    let key = match args.get("key") {
        Some(val) => try_get_value!("get", "key", String, val),
        None => return Err(Error::msg("The `get` filter has to have an `key` argument")),
    };

    match value.as_object() {
        Some(o) => match o.get(&key) {
            Some(val) => Ok(val.clone()),
            // If the value is not present, allow for an optional default value
            None => match default {
                Some(def) => Ok(def.clone()),
                None => Err(Error::msg(format!(
                    "Filter `get` tried to get key `{}` but it wasn't found",
                    &key
                ))),
            },
        },
        None => Err(Error::msg("Filter `get` was used on a value that isn't an object")),
    }
}
