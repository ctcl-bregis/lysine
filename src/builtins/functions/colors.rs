use std::collections::HashMap;
//use chrono::prelude::*;
//use rand::Rng;
use serde_json::value::{from_value, to_value, Value};
use rand::seq::SliceRandom;

use crate::errors::{Error, Result};

pub fn hex2rgb(args: &HashMap<String, Value>) -> Result<Value> {
    todo!();
}

pub fn randomcolor(args: &HashMap<String, Value>) -> Result<Value> {
    todo!();
}
