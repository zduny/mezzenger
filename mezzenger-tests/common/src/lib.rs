use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Message1 {
    Int(i32),
    String(String),
    Struct { a: i32, b: f64 },
}

pub fn messages1_part1() -> Vec<Message1> {
    vec![
        Message1::Int(2),
        Message1::String("Hello".to_string()),
        Message1::Struct { a: 2, b: 3.0 },
    ]
}

pub fn messages1_part2() -> Vec<Message1> {
    vec![
        Message1::Int(6),
        Message1::String("World".to_string()),
    ]
}

pub fn messages1_all() -> Vec<Message1> {
    vec![
        Message1::Int(2),
        Message1::String("Hello".to_string()),
        Message1::Struct { a: 2, b: 3.0 },
        Message1::Int(6),
        Message1::String("World".to_string()),
    ]
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum Message2 {
    Welcome { native_client: bool },
    Float(f32),
    String(String),
    Struct { a: i32, b: String },
}

pub fn messages2_part1() -> Vec<Message2> {
    vec![
        Message2::Float(2.0),
        Message2::String("Hello".to_string()),
    ]
}

pub fn messages2_part2() -> Vec<Message2> {
    vec![
        Message2::Struct { a: 2, b: "World".to_string() },
        Message2::Float(5.0),
        Message2::String("Hello again".to_string()),
    ]
}

pub fn messages2_all() -> Vec<Message2> {
    vec![
        Message2::Float(2.0),
        Message2::String("Hello".to_string()),
        Message2::Struct { a: 2, b: "World".to_string() },
        Message2::Float(5.0),
        Message2::String("Hello again".to_string()),
    ]
}
