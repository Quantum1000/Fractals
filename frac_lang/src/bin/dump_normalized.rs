use std::env;
use std::fs;
use std::process::ExitCode;

use frac_lang::normalizer::normalize;
use frac_lang::parser::parse;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: dump_normalized <path-to-.frac>");
        return ExitCode::from(2);
    }

    let source = match fs::read_to_string(&args[1]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {e}", args[1]);
            return ExitCode::from(2);
        }
    };

    let (file, parse_errors) = parse(&source);
    for e in &parse_errors {
        eprintln!("parse error: {}", e.message);
    }

    let (nf, norm_errors) = match normalize(file) {
        Ok(n) => (Some(n), Vec::new()),
        Err(errs) => (None, errs),
    };
    for e in &norm_errors {
        eprintln!("normalize error: {e}");
    }

    let nf = match nf {
        Some(n) => n,
        None => return ExitCode::from(1),
    };

    println!("=== tiles ===");
    let mut tile_names: Vec<&String> = nf.tiles.keys().collect();
    tile_names.sort();
    for name in tile_names {
        let t = &nf.tiles[name];
        println!("  {name}: symmetry={:?}, invariants={:?}", t.symmetry, t.invariants);
    }

    println!("=== partitions ===");
    let mut part_keys: Vec<&(String, String)> = nf.partitions.keys().collect();
    part_keys.sort();
    for key in part_keys {
        let p = &nf.partitions[key];
        println!("  {}.{}:", key.0, key.1);
        for c in &p.children {
            println!(
                "    child[{}] name={:?} tile_type={} anchor={} polygon={:?}",
                c.index, c.name, c.tile_type, c.anchor_vertex, c.polygon
            );
        }
    }

    ExitCode::SUCCESS
}
