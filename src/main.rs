use serde_json;
use std::env;

// Available if you need it!
// use serde_bencode

fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    if let Some(s) = encoded_value
        .split_once(":")
        .and_then(|(len, rest)| Some((len.parse::<usize>().ok()?, rest)))
        .map(|(len, rest)| &rest[..len])
    {
        s.into()
    } else if let Some(i) = encoded_value
        .strip_prefix('i')
        .and_then(|rest| rest.split_once('e'))
        .and_then(|(s,_)| s.parse::<i64>().ok())
    {
        i.into()
    } else {
        panic!("Unhandled encoded value: {}", encoded_value)
    }
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        // You can use print statements as follows for debugging, they'll be visible when running tests.
        eprintln!("Logs from your program will appear here!");

        // Uncomment this block to pass the first stage
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value.to_string());
    } else {
        eprintln!("unknown command: {}", args[1])
    }
}
