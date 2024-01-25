use serde_json;
use std::env;

// Available if you need it!
// use serde_bencode

fn parse_bencoded_string(input: &str) -> Option<(serde_json::Value, &str)> {
    input
        .split_once(":")
        .and_then(|(len, rest)| Some((len.parse::<usize>().ok()?, rest)))
        .map(|(len, rest)| ((&rest[..len]).into(), &rest[len..]))
}

fn parse_bencoded_i64(input: &str) -> Option<(serde_json::Value, &str)> {
    input
        .strip_prefix('i')
        .and_then(|rest| rest.split_once('e'))
        .and_then(|(s, rest)| Some((s.parse::<i64>().ok()?.into(), rest)))
}

fn parse_bencoded_value(input: &str) -> Option<(serde_json::Value, &str)> {
    match input.chars().next() {
        Some('i') => parse_bencoded_i64(input),
        Some('0'..='9') => parse_bencoded_string(input),
        Some('l') => {
            //eprintln!("parsing {input:?}");
            let mut input = &input[1..];
            let mut vec = vec![];
            while input.chars().next()? != 'e' {
                let (v, rem) = parse_bencoded_value(input)?;
                vec.push(v);
                input = rem;
            }
            Some((vec.into(), &input[1..]))
        },
        _ => None,
    }
}

fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    if let Some((v,_)) = parse_bencoded_value(encoded_value) {
        v
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
