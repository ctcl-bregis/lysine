// Filters operating on array
use std::collections::HashMap;

use crate::context::{dotted_pointer, ValueRender};
use crate::errors::{Error, Result};
use crate::filter_utils::{get_sort_strategy_for_type, get_unique_strategy_for_type};
use crate::utils::render_to_string;
use serde_json::value::{to_value, Map, Value};

// Returns the nth value of an array
// If the array is empty, returns empty string
pub fn nth(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let arr = try_get_value!("nth", "value", Vec<Value>, value);

    if arr.is_empty() {
        return Ok(to_value("").unwrap());
    }

    let index = match args.get("n") {
        Some(val) => try_get_value!("nth", "n", usize, val),
        None => return Err(Error::msg("The `nth` filter has to have an `n` argument")),
    };

    Ok(arr.get(index).unwrap_or(&to_value("").unwrap()).to_owned())
}

// Returns the first value of an array
// If the array is empty, returns empty string
pub fn first(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    let mut arr = try_get_value!("first", "value", Vec<Value>, value);

    if arr.is_empty() {
        Ok(to_value("").unwrap())
    } else {
        Ok(arr.swap_remove(0))
    }
}

// Returns the last value of an array
// If the array is empty, returns empty string
pub fn last(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    let mut arr = try_get_value!("last", "value", Vec<Value>, value);

    Ok(arr.pop().unwrap_or_else(|| to_value("").unwrap()))
}

// Joins all values in the array by the `sep` argument given
// If no separator is given, it will use `""` (empty string) as separator
// If the array is empty, returns empty string
pub fn join(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let arr = try_get_value!("join", "value", Vec<Value>, value);
    let sep = match args.get("sep") {
        Some(val) => {
            let s = try_get_value!("truncate", "sep", String, val);
            // When reading from a file, it will escape `\n` to `\\n` for example so we need
            // to replace double escape. In practice it might cause issues if someone wants to join
            // with `\\n` for real but that seems pretty unlikely
            s.replace("\\n", "\n").replace("\\t", "\t")
        }
        None => String::new(),
    };

    // Convert all the values to strings before we join them together.
    let rendered = arr
        .iter()
        .map(|v| render_to_string(|| "joining array".to_string(), |w| v.render(w)))
        .collect::<Result<Vec<_>>>()?;
    to_value(rendered.join(&sep)).map_err(Error::json)
}

// Sorts the array in ascending order.
// Use the 'attribute' argument to define a field to sort by.
pub fn sort(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let arr = try_get_value!("sort", "value", Vec<Value>, value);
    if arr.is_empty() {
        return Ok(arr.into());
    }

    let attribute = match args.get("attribute") {
        Some(val) => try_get_value!("sort", "attribute", String, val),
        None => String::new(),
    };

    let first = dotted_pointer(&arr[0], &attribute).ok_or_else(|| {
        Error::msg(format!("attribute '{}' does not reference a field", attribute))
    })?;

    let mut strategy = get_sort_strategy_for_type(first)?;
    for v in &arr {
        let key = dotted_pointer(v, &attribute).ok_or_else(|| {
            Error::msg(format!("attribute '{}' does not reference a field", attribute))
        })?;
        strategy.try_add_pair(v, key)?;
    }
    let sorted = strategy.sort();

    Ok(sorted.into())
}

// Remove duplicates from an array.
// Use the 'attribute' argument to define a field to filter on.
// For strings, use the 'case_sensitive' argument (defaults to false) to control the comparison.
pub fn unique(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let arr = try_get_value!("unique", "value", Vec<Value>, value);
    if arr.is_empty() {
        return Ok(arr.into());
    }

    let case_sensitive = match args.get("case_sensitive") {
        Some(val) => try_get_value!("unique", "case_sensitive", bool, val),
        None => false,
    };

    let attribute = match args.get("attribute") {
        Some(val) => try_get_value!("unique", "attribute", String, val),
        None => String::new(),
    };

    let first = dotted_pointer(&arr[0], &attribute).ok_or_else(|| {
        Error::msg(format!("attribute '{}' does not reference a field", attribute))
    })?;

    let disc = std::mem::discriminant(first);
    let mut strategy = get_unique_strategy_for_type(first, case_sensitive)?;

    let arr = arr
        .into_iter()
        .filter_map(|v| match dotted_pointer(&v, &attribute) {
            Some(key) => {
                if disc == std::mem::discriminant(key) {
                    match strategy.insert(key) {
                        Ok(false) => None,
                        Ok(true) => Some(Ok(v)),
                        Err(e) => Some(Err(e)),
                    }
                } else {
                    Some(Err(Error::msg("unique filter can't compare multiple types")))
                }
            }
            None => None,
        })
        .collect::<Result<Vec<_>>>();

    Ok(to_value(arr?).unwrap())
}

// Group the array values by the `attribute` given
// Returns a hashmap of key => values, items without the `attribute` or where `attribute` is `null` are discarded.
// The returned keys are stringified
pub fn group_by(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let arr = try_get_value!("group_by", "value", Vec<Value>, value);
    if arr.is_empty() {
        return Ok(Map::new().into());
    }

    let key = match args.get("attribute") {
        Some(val) => try_get_value!("group_by", "attribute", String, val),
        None => {
            return Err(Error::msg("The `group_by` filter has to have an `attribute` argument"))
        }
    };

    let mut grouped = Map::new();

    for val in arr {
        if let Some(key_val) = dotted_pointer(&val, &key).cloned() {
            if key_val.is_null() {
                continue;
            }

            let str_key = match key_val.as_str() {
                Some(key) => key.to_owned(),
                None => format!("{}", key_val),
            };

            if let Some(vals) = grouped.get_mut(&str_key) {
                vals.as_array_mut().unwrap().push(val);
                continue;
            }

            grouped.insert(str_key, Value::Array(vec![val]));
        }
    }

    Ok(to_value(grouped).unwrap())
}

// Filter the array values, returning only the values where the `attribute` is equal to the `value`
// Values without the `attribute` or with a null `attribute` are discarded
// If the `value` is not passed, discard all elements where the attribute is null.
pub fn filter(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let mut arr = try_get_value!("filter", "value", Vec<Value>, value);
    if arr.is_empty() {
        return Ok(arr.into());
    }

    let key = match args.get("attribute") {
        Some(val) => try_get_value!("filter", "attribute", String, val),
        None => return Err(Error::msg("The `filter` filter has to have an `attribute` argument")),
    };
    let value = args.get("value").unwrap_or(&Value::Null);

    arr = arr
        .into_iter()
        .filter(|v| {
            let val = dotted_pointer(v, &key).unwrap_or(&Value::Null);
            if value.is_null() {
                !val.is_null()
            } else {
                val == value
            }
        })
        .collect::<Vec<_>>();

    Ok(to_value(arr).unwrap())
}

// Map retrieves an attribute from a list of objects.
// The 'attribute' argument specifies what to retrieve.
pub fn map(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let arr = try_get_value!("map", "value", Vec<Value>, value);
    if arr.is_empty() {
        return Ok(arr.into());
    }

    let attribute = match args.get("attribute") {
        Some(val) => try_get_value!("map", "attribute", String, val),
        None => return Err(Error::msg("The `map` filter has to have an `attribute` argument")),
    };

    let arr = arr
        .into_iter()
        .filter_map(|v| match dotted_pointer(&v, &attribute) {
            Some(val) if !val.is_null() => Some(val.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    Ok(to_value(arr).unwrap())
}

#[inline]
fn get_index(i: f64, array: &[Value]) -> usize {
    if i >= 0.0 {
        i as usize
    } else {
        (array.len() as f64 + i) as usize
    }
}

// Slice the array
// Use the `start` argument to define where to start (inclusive, default to `0`)
// and `end` argument to define where to stop (exclusive, default to the length of the array)
// `start` and `end` are 0-indexed
pub fn slice(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let arr = try_get_value!("slice", "value", Vec<Value>, value);
    if arr.is_empty() {
        return Ok(arr.into());
    }

    let start = match args.get("start") {
        Some(val) => get_index(try_get_value!("slice", "start", f64, val), &arr),
        None => 0,
    };

    let mut end = match args.get("end") {
        Some(val) => get_index(try_get_value!("slice", "end", f64, val), &arr),
        None => arr.len(),
    };

    if end > arr.len() {
        end = arr.len();
    }

    // Not an error, but returns an empty Vec
    if start >= end {
        return Ok(Vec::<Value>::new().into());
    }

    Ok(arr[start..end].into())
}

// Concat the array with another one if the `with` parameter is an array or
// just append it otherwise
pub fn concat(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let mut arr = try_get_value!("concat", "value", Vec<Value>, value);

    let value = match args.get("with") {
        Some(val) => val,
        None => return Err(Error::msg("The `concat` filter has to have a `with` argument")),
    };

    if value.is_array() {
        match value {
            Value::Array(vals) => {
                for val in vals {
                    arr.push(val.clone());
                }
            }
            _ => unreachable!("Got something other than an array??"),
        }
    } else {
        arr.push(value.clone());
    }

    Ok(to_value(arr).unwrap())
}
