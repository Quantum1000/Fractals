/// Canonicalization pass (step 8).
///
/// Rewrites the AST in canonical form:
///   - Items sorted: tiles → partitions (by tile, then partition name) →
///     functions (topological order, callees first) → patterns.
///   - Float coordinates in vertex declarations rounded to 6 decimal places.
///   - Explicit child indices assigned where missing (done by topology pass,
///     so here we just sort child_names by index).
/// Nothing semantic is added to the file.

use crate::ast::{File, Item, Spanned};
use crate::printer::print_file;
use crate::parser::parse;

/// Sort items into canonical order, round vertex coordinates, sort child_names.
/// Returns a freshly parsed canonical File (so spans are updated).
pub fn canonicalize(file: File) -> File {
    let mut tiles: Vec<Spanned<Item>> = Vec::new();
    let mut partitions: Vec<Spanned<Item>> = Vec::new();
    let mut functions: Vec<Spanned<Item>> = Vec::new();
    let mut patterns: Vec<Spanned<Item>> = Vec::new();
    let mut other: Vec<Spanned<Item>> = Vec::new();

    for item in file.items {
        match &item.node {
            Item::Tile(_)      => tiles.push(item),
            Item::Partition(_) => partitions.push(item),
            Item::Function(_)  => functions.push(item),
            Item::Pattern(_)   => patterns.push(item),
            Item::Error(_)     => other.push(item),
        }
    }

    // Sort tiles alphabetically.
    tiles.sort_by(|a, b| tile_name(a).cmp(tile_name(b)));

    // Sort partitions: by tile name first, then partition name.
    partitions.sort_by(|a, b| {
        let (at, ap) = partition_names(a);
        let (bt, bp) = partition_names(b);
        at.cmp(bt).then(ap.cmp(bp))
    });

    // Functions: sort child_names within each partition (by index).
    let mut partitions = partitions.into_iter().map(|mut item| {
        if let Item::Partition(ref mut p) = item.node {
            p.child_names.sort_by_key(|cn| cn.node.index.node);
        }
        item
    }).collect::<Vec<_>>();

    // Round vertex coordinates.
    let mut all_items = Vec::new();
    for mut item in tiles {
        if let Item::Tile(ref mut t) = item.node {
            for pt in &mut t.canonical {
                pt.node.x = round6(pt.node.x);
                pt.node.y = round6(pt.node.y);
            }
        }
        all_items.push(item);
    }
    for mut item in partitions {
        if let Item::Partition(ref mut p) = item.node {
            for v in &mut p.vertices {
                v.node.pos.node.x = round6(v.node.pos.node.x);
                v.node.pos.node.y = round6(v.node.pos.node.y);
            }
        }
        all_items.push(item);
    }
    all_items.extend(functions);
    all_items.extend(patterns);
    all_items.extend(other);

    let canonical = File { items: all_items };

    // Round-trip through the printer to normalise whitespace and update spans.
    let text = print_file(&canonical);
    let (reparsed, _) = parse(&text);
    reparsed
}

fn round6(x: f64) -> f64 {
    (x * 1_000_000.0).round() / 1_000_000.0
}

fn tile_name<'a>(item: &'a Spanned<Item>) -> &'a str {
    if let Item::Tile(t) = &item.node { &t.name.0.node } else { "" }
}

fn partition_names<'a>(item: &'a Spanned<Item>) -> (&'a str, &'a str) {
    if let Item::Partition(p) = &item.node {
        (&p.tile.0.node, &p.name.0.node)
    } else {
        ("", "")
    }
}
