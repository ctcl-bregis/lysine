// Filters operating on multiple types
use std::collections::HashMap;
#[cfg(feature = "date-locale")]
use std::convert::TryFrom;
use std::iter::FromIterator;

use crate::errors::{Error, Result};
use crate::utils::render_to_string;

use chrono::{
    format::{Item, StrftimeItems},
    DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc,
};

use chrono_tz::Tz;
use serde_json::value::{to_value, Value};
use serde_json::{to_string, to_string_pretty};

use crate::context::ValueRender;

// Returns the number of items in an array or an object, or the number of characters in a string.
pub fn length(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    match value {
        Value::Array(arr) => Ok(to_value(arr.len()).unwrap()),
        Value::Object(m) => Ok(to_value(m.len()).unwrap()),
        Value::String(s) => Ok(to_value(s.chars().count()).unwrap()),
        _ => Err(Error::msg(
            "Filter `length` was used on a value that isn't an array, an object, or a string.",
        )),
    }
}

// Reverses the elements of an array or the characters in a string.
pub fn reverse(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    match value {
        Value::Array(arr) => {
            let mut rev = arr.clone();
            rev.reverse();
            to_value(&rev).map_err(Error::json)
        }
        Value::String(s) => to_value(String::from_iter(s.chars().rev())).map_err(Error::json),
        _ => Err(Error::msg(format!(
            "Filter `reverse` received an incorrect type for arg `value`: \
             got `{}` but expected Array|String",
            value
        ))),
    }
}

// Encodes a value of any type into json, optionally `pretty`-printing it
// `pretty` can be true to enable pretty-print, or omitted for compact printing
pub fn json_encode(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let pretty = args.get("pretty").and_then(Value::as_bool).unwrap_or(false);

    if pretty {
        to_string_pretty(&value).map(Value::String).map_err(Error::json)
    } else {
        to_string(&value).map(Value::String).map_err(Error::json)
    }
}

// Returns a formatted time according to the given `format` argument.
// `format` defaults to the ISO 8601 `YYYY-MM-DD` format.
//
// Input can be an i64 timestamp (seconds since epoch) or an RFC3339 string
// (default serialization format for `chrono::DateTime`).
//
// a full reference for the time formatting syntax is available
// on [chrono docs](https://lifthrasiir.github.io/rust-chrono/chrono/format/strftime/index.html)

pub fn date(value: &Value, args: &HashMap<String, Value>) -> Result<Value> {
    let format = match args.get("format") {
        Some(val) => try_get_value!("date", "format", String, val),
        None => "%Y-%m-%d".to_string(),
    };

    let items: Vec<Item> =
        StrftimeItems::new(&format).filter(|item| matches!(item, Item::Error)).collect();
    if !items.is_empty() {
        return Err(Error::msg(format!("Invalid date format `{}`", format)));
    }

    let timezone = match args.get("timezone") {
        Some(val) => {
            let timezone = try_get_value!("date", "timezone", String, val);
            match timezone.parse::<Tz>() {
                Ok(timezone) => Some(timezone),
                Err(_) => {
                    return Err(Error::msg(format!("Error parsing `{}` as a timezone", timezone)))
                }
            }
        }
        None => None,
    };

    #[cfg(feature = "date-locale")]
    let formatted = {
        let locale = match args.get("locale") {
            Some(val) => {
                let locale = try_get_value!("date", "locale", String, val);
                chrono::Locale::try_from(locale.as_str())
                    .map_err(|_| Error::msg(format!("Error parsing `{}` as a locale", locale)))?
            }
            None => chrono::Locale::POSIX,
        };
        match value {
            Value::Number(n) => match n.as_i64() {
                Some(i) => {
                    let date = NaiveDateTime::from_timestamp_opt(i, 0).expect(
                        "out of bound seconds should not appear, as we set nanoseconds to zero",
                    );
                    match timezone {
                        Some(timezone) => {
                            timezone.from_utc_datetime(&date).format_localized(&format, locale)
                        }
                        None => date.format(&format),
                    }
                }
                None => {
                    return Err(Error::msg(format!("Filter `date` was invoked on a float: {}", n)))
                }
            },
            Value::String(s) => {
                if s.contains('T') {
                    match s.parse::<DateTime<FixedOffset>>() {
                        Ok(val) => match timezone {
                            Some(timezone) => {
                                val.with_timezone(&timezone).format_localized(&format, locale)
                            }
                            None => val.format_localized(&format, locale),
                        },
                        Err(_) => match s.parse::<NaiveDateTime>() {
                            Ok(val) => DateTime::<Utc>::from_naive_utc_and_offset(val, Utc)
                                .format_localized(&format, locale),
                            Err(_) => {
                                return Err(Error::msg(format!(
                                    "Error parsing `{:?}` as rfc3339 date or naive datetime",
                                    s
                                )));
                            }
                        },
                    }
                } else {
                    match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                        Ok(val) => DateTime::<Utc>::from_naive_utc_and_offset(
                            val.and_hms_opt(0, 0, 0).expect(
                                "out of bound should not appear, as we set the time to zero",
                            ),
                            Utc,
                        )
                        .format_localized(&format, locale),
                        Err(_) => {
                            return Err(Error::msg(format!(
                                "Error parsing `{:?}` as YYYY-MM-DD date",
                                s
                            )));
                        }
                    }
                }
            }
            _ => {
                return Err(Error::msg(format!(
                    "Filter `date` received an incorrect type for arg `value`: \
                     got `{:?}` but expected i64|u64|String",
                    value
                )));
            }
        }
    };

    #[cfg(not(feature = "date-locale"))]
    let formatted = match value {
        Value::Number(n) => match n.as_i64() {
            Some(i) => {
                let date = DateTime::from_timestamp(i, 0).expect("out of bound seconds should not appear, as nanoseconds are set to zero");
                match timezone {
                    Some(timezone) => timezone.from_utc_datetime(&date.naive_utc()).format(&format),
                    None => date.format(&format),
                }
            }
            None => return Err(Error::msg(format!("Filter `date` was invoked on a float: {}", n))),
        },
        Value::String(s) => {
            if s.contains('T') {
                match s.parse::<DateTime<FixedOffset>>() {
                    Ok(val) => match timezone {
                        Some(timezone) => val.with_timezone(&timezone).format(&format),
                        None => val.format(&format),
                    },
                    Err(_) => match s.parse::<NaiveDateTime>() {
                        Ok(val) => {
                            DateTime::<Utc>::from_naive_utc_and_offset(val, Utc).format(&format)
                        }
                        Err(_) => {
                            return Err(Error::msg(format!(
                                "Error parsing `{:?}` as RFC3339 date or naive datetime",
                                s
                            )));
                        }
                    },
                }
            } else {
                match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
                    Ok(val) => DateTime::<Utc>::from_naive_utc_and_offset(
                        val.and_hms_opt(0, 0, 0)
                            .expect("out of bound should not appear, as we set the time to zero"),
                        Utc,
                    )
                    .format(&format),
                    Err(_) => {
                        return Err(Error::msg(format!(
                            "Error parsing `{:?}` as YYYY-MM-DD date",
                            s
                        )));
                    }
                }
            }
        }
        _ => {
            return Err(Error::msg(format!(
                "Filter `date` received an incorrect type for arg `value`: \
                 got `{:?}` but expected i64|u64|String",
                value
            )));
        }
    };

    to_value(formatted.to_string()).map_err(Error::json)
}

// Returns the given value as a string.
pub fn as_str(value: &Value, _: &HashMap<String, Value>) -> Result<Value> {
    let value =
        render_to_string(|| format!("as_str for value of kind {}", value), |w| value.render(w))?;
    to_value(value).map_err(Error::json)
}

