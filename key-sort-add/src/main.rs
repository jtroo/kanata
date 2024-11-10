//! one:
//!
//! Takes a file formatted as:
//!
//!     KEY_RESERVED = 0,
//!     KEY_ESC = 1,
//!     KEY_1 = 2,
//!     KEY_2 = 3,
//!     KEY_3 = 4,
//!     KEY_4 = 5,
//!     ...
//!
//! Outputs to stdout a sorted version of the file with numeric gaps filled in with:
//!
//!     KEY_X = X,
//!
//! two: mapping.txt to ensure KeyCode and OsCode can simply be transmuted into each other.

use std::io::Read;

fn main() {
    match std::env::args().nth(1).expect("function parameter").as_str() {
        "one" => one(),
        "two" => two(),
        _ => panic!("unknown capabality"),
    }
}

fn one() {
    let mut f = std::fs::File::open(std::env::args().nth(2).expect("filename parameter"))
        .expect("file open");
    let mut s = String::new();
    f.read_to_string(&mut s).expect("read file");
    let mut keys = s
        .lines()
        .map(|l| {
            let mut segments = l.trim_end_matches(',').trim().split(" = ");
            let key = segments.next().expect("a string");
            let num: u16 = u16::from_str_radix(
                segments
                    .next()
                    .map(|s| s.trim_start_matches("0x"))
                    .expect("string after ="),
                10,
            )
            .expect("u16");
            (key.to_owned(), num)
        })
        .collect::<Vec<_>>();
    keys.sort_by_key(|k| k.1);
    let mut keys_to_add = vec![];
    let mut cur_key = keys.iter();
    let mut prev_key = keys.iter();
    cur_key.next();
    for cur in cur_key {
        let prev = prev_key.next().expect("lagging iterator is valid");
        for missing in prev.1 + 1..cur.1 {
            keys_to_add.push((format!("K{missing}"), missing));
        }
    }
    keys.append(&mut keys_to_add);
    keys.sort_by_key(|k| k.1);
    for key in keys {
        println!("{} = {},", key.0, key.1);
    }
}

fn two() {
    use std::collections::HashMap;

    let mut f = std::fs::File::open(std::env::args().nth(2).expect("filename parameter"))
        .expect("file open");
    let mut s = String::new();
    f.read_to_string(&mut s).expect("read file");
    let mut lines = s.lines();

    // filter out useless lines
    while let Some(line) = lines.next() {
        if line == "=== kc to osc" {
            break;
        }
    }

    // parse kc to osc
    let mut kc_to_osc: HashMap<&str, &str> = HashMap::new();
    while let Some(line) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        if line == "=== osc to u16" {
            break;
        }
        let (kc, osc) = line.split_once(" => ").expect("arrow separator");
        let kc = kc.trim_start_matches("KeyCode::");
        let osc = osc.trim_end_matches(',')
                .trim_start_matches("OsCode::");
        kc_to_osc.insert(kc, osc);
    }

    // parse osc to u16
    let mut osc_vals: HashMap<&str, u16> = HashMap::new();
    while let Some(line) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        if line == "=== all kcs" {
            break;
        }
        let (kc, num) = line.split_once(" = ").expect("equal separator");
        let num = num.trim_end_matches(',').parse::<u16>().expect("u16");
        osc_vals.insert(kc, num);
    }

    // parse kcs
    let mut kc_vals: Vec<(&str, Option<u16>)> = vec![];
    while let Some(line) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        let kc = line.trim_end_matches(',');
        let val: Option<u16> = kc_to_osc.get(&kc)
            .and_then(|osc| osc_vals.get(osc))
            .copied();
        kc_vals.push((kc, val));
    }

    for (kc, val) in kc_vals.iter() {
        println!("{kc} = {},", val.unwrap_or(65535));
    }
}
