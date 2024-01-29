use serde_json;

pub fn parse_bencoded_string(input: &str) -> Option<(serde_json::Value, &str)> {
    input
        .split_once(":")
        .and_then(|(len, rest)| Some((len.parse::<usize>().ok()?, rest)))
        .map(|(len, rest)| ((&rest[..len]).into(), &rest[len..]))
}

pub fn parse_bencoded_i64(input: &str) -> Option<(serde_json::Value, &str)> {
    input
        .strip_prefix('i')
        .and_then(|rest| rest.split_once('e'))
        .and_then(|(s, rest)| Some((s.parse::<i64>().ok()?.into(), rest)))
}

pub fn parse_bencoded_value(input: &str) -> Option<(serde_json::Value, &str)> {
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
        }
        Some('d') => {
            let mut input = &input[1..];
            let mut d = serde_json::Map::new();
            while input.chars().next()? != 'e' {
                let (key, rest) = parse_bencoded_string(input)?;
                let (val, rest) = parse_bencoded_value(rest)?;
                if let serde_json::Value::String(key) = key {
                    d.insert(key, val);
                }
                input = rest;
            }
            Some((d.into(), &input[1..]))
        }
        _ => None,
    }
}

pub fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    if let Some((v, _)) = parse_bencoded_value(encoded_value) {
        v
    } else {
        panic!("Unhandled encoded value: {}", encoded_value)
    }
}