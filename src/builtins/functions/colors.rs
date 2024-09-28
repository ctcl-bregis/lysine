use std::collections::HashMap;
//use chrono::prelude::*;
//use rand::Rng;
use serde_json::value::{from_value, to_value, Value};
use rand::seq::SliceRandom;

use crate::errors::{Error, Result};

pub fn hex2rgb(args: &HashMap<String, Value>) -> Result<Value> {
    let random = match args.get("array") {
        Some(val) => match val {
            Value::Array(vec) => {
                vec.choose(&mut rand::thread_rng()).unwrap()
            },
            _ => return Err(Error::msg(format!(
                "Function `now` received utc={:?} but `array` can only be an array",
                val
            ))),
        },
        None => return Err(Error::msg("Function `pick_random` was called without a `array` argument")),
    };

    Ok(random.clone())
}
