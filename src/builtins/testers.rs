use crate::context::ValueNumber;
use crate::errors::{Error, Result};
use regex::Regex;
use serde_json::value::Value;

// The tester function type definition
pub trait Test: Sync + Send {
    // The tester function type definition
    fn test(&self, value: Option<&Value>, args: &[Value]) -> Result<bool>;
}

impl<F> Test for F
where
    F: Fn(Option<&Value>, &[Value]) -> Result<bool> + Sync + Send,
{
    fn test(&self, value: Option<&Value>, args: &[Value]) -> Result<bool> {
        self(value, args)
    }
}

// Check that the number of args match what was expected
pub fn number_args_allowed(tester_name: &str, max: usize, args_len: usize) -> Result<()> {
    if max == 0 && args_len > max {
        return Err(Error::msg(format!(
            "Tester `{}` was called with some args but this test doesn't take args",
            tester_name
        )));
    }

    if args_len > max {
        return Err(Error::msg(format!(
            "Tester `{}` was called with {} args, the max number is {}",
            tester_name, args_len, max
        )));
    }

    Ok(())
}

// Called to check if the Value is defined and return an Err if not
pub fn value_defined(tester_name: &str, value: Option<&Value>) -> Result<()> {
    if value.is_none() {
        return Err(Error::msg(format!(
            "Tester `{}` was called on an undefined variable",
            tester_name
        )));
    }

    Ok(())
}

// Helper function to extract string from an [`Option<Value>`] to remove boilerplate
// with tester error handling
pub fn extract_string<'a>(
    tester_name: &str,
    part: &str,
    value: Option<&'a Value>,
) -> Result<&'a str> {
    match value.and_then(Value::as_str) {
        Some(s) => Ok(s),
        None => Err(Error::msg(format!(
            "Tester `{}` was called {} that isn't a string",
            tester_name, part
        ))),
    }
}

// Returns true if `value` is defined. Otherwise, returns false.
pub fn defined(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("defined", 0, params.len())?;

    Ok(value.is_some())
}

// Returns true if `value` is undefined. Otherwise, returns false.
pub fn undefined(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("undefined", 0, params.len())?;

    Ok(value.is_none())
}

// Returns true if `value` is a string. Otherwise, returns false.
pub fn string(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("string", 0, params.len())?;
    value_defined("string", value)?;

    match value {
        Some(Value::String(_)) => Ok(true),
        _ => Ok(false),
    }
}

// Returns true if `value` is a number. Otherwise, returns false.
pub fn number(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("number", 0, params.len())?;
    value_defined("number", value)?;

    match value {
        Some(Value::Number(_)) => Ok(true),
        _ => Ok(false),
    }
}

// Returns true if `value` is an odd number. Otherwise, returns false.
pub fn odd(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("odd", 0, params.len())?;
    value_defined("odd", value)?;

    match value.and_then(|v| v.to_number().ok()) {
        Some(f) => Ok(f % 2.0 != 0.0),
        _ => Err(Error::msg("Tester `odd` was called on a variable that isn't a number")),
    }
}

// Returns true if `value` is an even number. Otherwise, returns false.
pub fn even(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("even", 0, params.len())?;
    value_defined("even", value)?;

    let is_odd = odd(value, params)?;
    Ok(!is_odd)
}

// Returns true if `value` is divisible by the first param. Otherwise, returns false.
pub fn divisible_by(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("divisibleby", 1, params.len())?;
    value_defined("divisibleby", value)?;

    match value.and_then(|v| v.to_number().ok()) {
        Some(val) => match params.first().and_then(|v| v.to_number().ok()) {
            Some(p) => Ok(val % p == 0.0),
            None => Err(Error::msg(
                "Tester `divisibleby` was called with a parameter that isn't a number",
            )),
        },
        None => {
            Err(Error::msg("Tester `divisibleby` was called on a variable that isn't a number"))
        }
    }
}

// Returns true if `value` can be iterated over in Lysine (ie is an array/tuple or an object).
// Otherwise, returns false.
pub fn iterable(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("iterable", 0, params.len())?;
    value_defined("iterable", value)?;

    Ok(value.unwrap().is_array() || value.unwrap().is_object())
}

// Returns true if the given variable is an object (ie can be iterated over key, value).
// Otherwise, returns false.
pub fn object(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("object", 0, params.len())?;
    value_defined("object", value)?;

    Ok(value.unwrap().is_object())
}

// Returns true if `value` starts with the given string. Otherwise, returns false.
pub fn starting_with(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("starting_with", 1, params.len())?;
    value_defined("starting_with", value)?;

    let value = extract_string("starting_with", "on a variable", value)?;
    let needle = extract_string("starting_with", "with a parameter", params.first())?;
    Ok(value.starts_with(needle))
}

// Returns true if `value` ends with the given string. Otherwise, returns false.
pub fn ending_with(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("ending_with", 1, params.len())?;
    value_defined("ending_with", value)?;

    let value = extract_string("ending_with", "on a variable", value)?;
    let needle = extract_string("ending_with", "with a parameter", params.first())?;
    Ok(value.ends_with(needle))
}

// Returns true if `value` contains the given argument. Otherwise, returns false.
pub fn containing(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("containing", 1, params.len())?;
    value_defined("containing", value)?;

    match value.unwrap() {
        Value::String(v) => {
            let needle = extract_string("containing", "with a parameter", params.first())?;
            Ok(v.contains(needle))
        }
        Value::Array(v) => Ok(v.contains(params.first().unwrap())),
        Value::Object(v) => {
            let needle = extract_string("containing", "with a parameter", params.first())?;
            Ok(v.contains_key(needle))
        }
        _ => Err(Error::msg("Tester `containing` can only be used on string, array or map")),
    }
}

// Returns true if `value` is a string and matches the regex in the argument. Otherwise, returns false.
pub fn matching(value: Option<&Value>, params: &[Value]) -> Result<bool> {
    number_args_allowed("matching", 1, params.len())?;
    value_defined("matching", value)?;

    let value = extract_string("matching", "on a variable", value)?;
    let regex = extract_string("matching", "with a parameter", params.first())?;

    let regex = match Regex::new(regex) {
        Ok(regex) => regex,
        Err(err) => {
            return Err(Error::msg(format!(
                "Tester `matching`: Invalid regular expression: {}",
                err
            )));
        }
    };

    Ok(regex.is_match(value))
}
