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

use std::io::Read;

fn main() {
    let mut f = std::fs::File::open(std::env::args().nth(1).expect("filename parameter"))
        .expect("file open");
    let mut s = String::new();
    f.read_to_string(&mut s).expect("read file");
    let mut keys = s
        .lines()
        .map(|l| {
            let mut segments = l.trim_end_matches(',').trim().split(" = ");
            let key = segments.next().expect("a string");
            let num: u16 = str::parse(segments.next().expect("string after =")).expect("u16");
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
            keys_to_add.push((format!("KEY_{missing}"), missing));
        }
    }
    keys.append(&mut keys_to_add);
    keys.sort_by_key(|k| k.1);
    for key in keys {
        println!("{} = {},", key.0, key.1);
    }
}
