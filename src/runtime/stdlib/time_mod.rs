//! Time utilities

use super::super::{Value, RuntimeError};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn call(
    method: &str,
    args: Vec<Value>,
    _kwargs: HashMap<String, Value>,
) -> Result<Value, RuntimeError> {
    match method {
        "now" => now(),
        "timestamp" => timestamp(),
        "sleep" => sleep_ms(&args),
        "elapsed" => elapsed(&args),
        "format" => format_time(&args),
        _ => Err(RuntimeError {
            message: format!("time.{}() not found", method),
        }),
    }
}

fn now() -> Result<Value, RuntimeError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| RuntimeError {
            message: format!("Time error: {}", e),
        })?;
    Ok(Value::Float(duration.as_secs_f64()))
}

fn timestamp() -> Result<Value, RuntimeError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| RuntimeError {
            message: format!("Time error: {}", e),
        })?;
    Ok(Value::Int(duration.as_secs() as i64))
}

fn sleep_ms(args: &[Value]) -> Result<Value, RuntimeError> {
    let ms = match args.first() {
        Some(Value::Int(n)) => {
            if *n < 0 || *n > 60_000 {
                return Err(RuntimeError {
                    message: "time.sleep() value must be 0-60000 ms".to_string(),
                });
            }
            *n as u64
        }
        _ => {
            return Err(RuntimeError {
                message: "time.sleep() requires milliseconds (integer)".to_string(),
            })
        }
    };
    std::thread::sleep(std::time::Duration::from_millis(ms));
    Ok(Value::Null)
}

fn elapsed(args: &[Value]) -> Result<Value, RuntimeError> {
    let start = match args.first() {
        Some(Value::Float(f)) => *f,
        Some(Value::Int(n)) => *n as f64,
        _ => {
            return Err(RuntimeError {
                message: "time.elapsed() requires a start timestamp".to_string(),
            })
        }
    };
    let now_val = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| RuntimeError {
            message: format!("Time error: {}", e),
        })?
        .as_secs_f64();
    Ok(Value::Float(now_val - start))
}

fn format_time(args: &[Value]) -> Result<Value, RuntimeError> {
    let ts = match args.first() {
        Some(Value::Float(f)) => *f,
        Some(Value::Int(n)) => *n as f64,
        _ => {
            // No arg = format current time
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| RuntimeError {
                    message: format!("Time error: {}", e),
                })?
                .as_secs_f64()
        }
    };

    let secs = ts as i64;
    let time_of_day = ((secs % 86400) + 86400) % 86400; // handle negative
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let days = if secs >= 0 {
        secs / 86400
    } else {
        (secs - 86399) / 86400
    };
    let (year, month, day) = epoch_days_to_date(days);

    Ok(Value::Str(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, month, day, hours, minutes, seconds
    )))
}

/// Civil calendar from day count (algorithm from Howard Hinnant)
fn epoch_days_to_date(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
