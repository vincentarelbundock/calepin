use std::io::{BufRead, BufReader, Write};
use std::{env, fs, path};

fn main() {
    let csv_path = path::Path::new("color_dict.csv");
    println!("cargo:rerun-if-changed={}", csv_path.display());

    let file = fs::File::open(csv_path).expect("color_dict.csv not found");
    let reader = BufReader::new(file);

    let mut entries: Vec<(String, String)> = Vec::new();
    for line in reader.lines().skip(1) {
        let line = line.unwrap();
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() >= 2 {
            let name = fields[0].trim_matches('"');
            let hex = fields[1].trim_matches('"');
            entries.push((name.to_string(), hex.to_string()));
        }
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = path::Path::new(&out_dir).join("colors.rs");
    let mut out = fs::File::create(out_path).unwrap();

    writeln!(out, "static COLORS: &[(&str, &str)] = &[").unwrap();
    for (name, hex) in &entries {
        writeln!(out, "    (\"{}\", \"{}\"),", name, hex).unwrap();
    }
    writeln!(out, "];").unwrap();
}
