use std::fs;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LogRecord<M> {
    pub time: DateTime<Utc>,
    pub message: M,
}

// const FILE: &str = include_str!("./incoming-messages-port-8003-test.json");
const FILE: &str = include_str!("./incoming-messages-27-01-written-steps.json");

fn main() {
    let mut new_json = String::new();
    for (line, json_string) in FILE.lines().into_iter().enumerate() {
        let parsed_log_binary =
            serde_json::from_str::<LogRecord<Vec<u8>>>(json_string).expect(&format!("Line {line} error"));

        let message = protocol::SendMessage::from_radio(&parsed_log_binary.message)
            .expect(&format!("Line {line} from radio binary error"));

        let json_line =
            serde_json::to_string(&message).expect(&format!("Line {line} to parsed json error"));
        new_json.push_str(&json_line);
        new_json.push('\n');
    }

    fs::write("./parsed-27-01-written-steps.json", new_json).unwrap()
}
