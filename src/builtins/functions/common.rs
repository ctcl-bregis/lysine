use std::collections::HashMap;
use chrono::prelude::*;
use rand::Rng;
use serde_json::value::{from_value, to_value, Value};
use rand::seq::SliceRandom;

use crate::errors::{Error, Result};

pub fn range(args: &HashMap<String, Value>) -> Result<Value> {
    let start = match args.get("start") {
        Some(val) => match from_value::<usize>(val.clone()) {
            Ok(v) => v,
            Err(_) => {
                return Err(Error::msg(format!(
                    "Function `range` received start={} but `start` can only be a number",
                    val
                )));
            }
        },
        None => 0,
    };
    let step_by = match args.get("step_by") {
        Some(val) => match from_value::<usize>(val.clone()) {
            Ok(v) => v,
            Err(_) => {
                return Err(Error::msg(format!(
                    "Function `range` received step_by={} but `step` can only be a number",
                    val
                )));
            }
        },
        None => 1,
    };
    let end = match args.get("end") {
        Some(val) => match from_value::<usize>(val.clone()) {
            Ok(v) => v,
            Err(_) => {
                return Err(Error::msg(format!(
                    "Function `range` received end={} but `end` can only be a number",
                    val
                )));
            }
        },
        None => {
            return Err(Error::msg("Function `range` was called without a `end` argument"));
        }
    };

    if start > end {
        return Err(Error::msg(
            "Function `range` was called with a `start` argument greater than the `end` one",
        ));
    }

    let mut i = start;
    let mut res = vec![];
    while i < end {
        res.push(i);
        i += step_by;
    }
    Ok(to_value(res).unwrap())
}

pub fn pick_random(args: &HashMap<String, Value>) -> Result<Value> {
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


pub fn now(args: &HashMap<String, Value>) -> Result<Value> {
    let utc = match args.get("utc") {
        Some(val) => match from_value::<bool>(val.clone()) {
            Ok(v) => v,
            Err(_) => {
                return Err(Error::msg(format!(
                    "Function `now` received utc={} but `utc` can only be a boolean",
                    val
                )));
            }
        },
        None => false,
    };
    let timestamp = match args.get("timestamp") {
        Some(val) => match from_value::<bool>(val.clone()) {
            Ok(v) => v,
            Err(_) => {
                return Err(Error::msg(format!(
                    "Function `now` received timestamp={} but `timestamp` can only be a boolean",
                    val
                )));
            }
        },
        None => false,
    };

    if utc {
        let datetime = Utc::now();
        if timestamp {
            return Ok(to_value(datetime.timestamp()).unwrap());
        }
        Ok(to_value(datetime.to_rfc3339()).unwrap())
    } else {
        let datetime = Local::now();
        if timestamp {
            return Ok(to_value(datetime.timestamp()).unwrap());
        }
        Ok(to_value(datetime.to_rfc3339()).unwrap())
    }
}

pub fn throw(args: &HashMap<String, Value>) -> Result<Value> {
    match args.get("message") {
        Some(val) => match from_value::<String>(val.clone()) {
            Ok(v) => Err(Error::msg(v)),
            Err(_) => Err(Error::msg(format!(
                "Function `throw` received message={} but `message` can only be a string",
                val
            ))),
        },
        None => Err(Error::msg("Function `throw` was called without a `message` argument")),
    }
}


pub fn random_int(args: &HashMap<String, Value>) -> Result<Value> {
    let start = match args.get("start") {
        Some(val) => match from_value::<isize>(val.clone()) {
            Ok(v) => v,
            Err(_) => {
                return Err(Error::msg(format!(
                    "Function `random_int` received start={} but `start` can only be a number",
                    val
                )));
            }
        },
        None => 0,
    };

    let end = match args.get("end") {
        Some(val) => match from_value::<isize>(val.clone()) {
            Ok(v) => v,
            Err(_) => {
                return Err(Error::msg(format!(
                    "Function `random_int` received end={} but `end` can only be a number",
                    val
                )));
            }
        },
        None => return Err(Error::msg("Function `random_int` didn't receive an `end` argument")),
    };
    let mut rng = rand::thread_rng();
    let res = rng.gen_range(start..end);

    Ok(Value::Number(res.into()))
}

pub fn get_env(args: &HashMap<String, Value>) -> Result<Value> {
    let name = match args.get("name") {
        Some(val) => match from_value::<String>(val.clone()) {
            Ok(v) => v,
            Err(_) => {
                return Err(Error::msg(format!(
                    "Function `get_env` received name={} but `name` can only be a string",
                    val
                )));
            }
        },
        None => return Err(Error::msg("Function `get_env` didn't receive a `name` argument")),
    };

    match std::env::var(&name).ok() {
        Some(res) => Ok(Value::String(res)),
        None => match args.get("default") {
            Some(default) => Ok(default.clone()),
            None => Err(Error::msg(format!("Environment variable `{}` not found", &name))),
        },
    }
}

