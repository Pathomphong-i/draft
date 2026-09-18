//! Formatted output and JSON serialization helper.

use serde::Serialize;

pub fn print_output<T: Serialize>(json: bool, json_val: &T, text_val: &str) {
    if json {
        if let Ok(serialized) = serde_json::to_string_pretty(json_val) {
            println!("{}", serialized);
        } else {
            eprintln!("Error serializing JSON output");
        }
    } else if !text_val.is_empty() {
        println!("{}", text_val);
    }
}
