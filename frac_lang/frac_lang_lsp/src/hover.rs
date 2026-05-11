use frac_lang::ast::{
    ChildRef, File, Item, RuleBody, Span, Spanned, Substitution, SymmetryGroup,
};
use frac_lang::normalizer::{ChildInfo, NormalizedFile, NormalizedTile};
use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use crate::document::Document;

pub fn hover(doc: &Document, position: Position) -> Option<Hover> {
    let offset = doc.line_index.offset_of_position(position);
    let target = find_hover_target(&doc.file, offset)?;
    let nf = doc.normalized.as_ref();
    let (markdown, span) = render(&target, nf)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown,
        }),
        range: Some(doc.line_index.range_of_span(span)),
    })
}

#[derive(Debug)]
enum HoverTarget {
    Tile { name: String, span: Span },
    Partition { tile: String, name: String, span: Span },
    Child { tile: String, partition: String, child_ref: ChildRefKind, span: Span },
}

#[derive(Debug)]
enum ChildRefKind {
    Index(u32),
    Name(String),
}

fn contains(s: Span, offset: usize) -> bool {
    offset >= s.start && offset <= s.end
}

fn find_hover_target(file: &File, offset: usize) -> Option<HoverTarget> {
    for item in &file.items {
        if !contains(item.span, offset) { continue; }
        match &item.node {
            Item::Tile(t) => {
                if contains(t.name.0.span, offset) {
                    return Some(HoverTarget::Tile {
                        name: t.name.0.node.clone(),
                        span: t.name.0.span,
                    });
                }
            }
            Item::Partition(p) => {
                if contains(p.tile.0.span, offset) {
                    return Some(HoverTarget::Tile {
                        name: p.tile.0.node.clone(),
                        span: p.tile.0.span,
                    });
                }
                if contains(p.name.0.span, offset) {
                    return Some(HoverTarget::Partition {
                        tile: p.tile.0.node.clone(),
                        name: p.name.0.node.clone(),
                        span: p.name.0.span,
                    });
                }
            }
            Item::Pattern(pat) => {
                if contains(pat.root.0.span, offset) {
                    return Some(HoverTarget::Tile {
                        name: pat.root.0.node.clone(),
                        span: pat.root.0.span,
                    });
                }
                for rule in &pat.rules {
                    if !contains(rule.span, offset) { continue; }
                    if contains(rule.node.tile.0.span, offset) {
                        return Some(HoverTarget::Tile {
                            name: rule.node.tile.0.node.clone(),
                            span: rule.node.tile.0.span,
                        });
                    }
                    if let Some(t) = find_in_rule_body(&rule.node.body, offset) {
                        return Some(t);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_in_rule_body(body: &Spanned<RuleBody>, offset: usize) -> Option<HoverTarget> {
    if !contains(body.span, offset) { return None; }
    match &body.node {
        RuleBody::If { then_branch, else_branch, .. } => {
            find_in_rule_body(then_branch, offset).or_else(|| find_in_rule_body(else_branch, offset))
        }
        RuleBody::Substitute(sub) => find_in_substitution(sub, offset),
    }
}

fn find_in_substitution(sub: &Spanned<Substitution>, offset: usize) -> Option<HoverTarget> {
    if !contains(sub.span, offset) { return None; }
    let s = &sub.node;
    if contains(s.tile.0.span, offset) {
        return Some(HoverTarget::Tile {
            name: s.tile.0.node.clone(),
            span: s.tile.0.span,
        });
    }
    if contains(s.partition.0.span, offset) {
        return Some(HoverTarget::Partition {
            tile: s.tile.0.node.clone(),
            name: s.partition.0.node.clone(),
            span: s.partition.0.span,
        });
    }
    for inj in &s.child_injections {
        if contains(inj.node.child.span, offset) {
            let kind = match &inj.node.child.node {
                ChildRef::Index(i) => ChildRefKind::Index(*i),
                ChildRef::Name(n) => ChildRefKind::Name(n.0.node.clone()),
            };
            return Some(HoverTarget::Child {
                tile: s.tile.0.node.clone(),
                partition: s.partition.0.node.clone(),
                child_ref: kind,
                span: inj.node.child.span,
            });
        }
    }
    None
}

fn render(target: &HoverTarget, nf: Option<&NormalizedFile>) -> Option<(String, Span)> {
    match target {
        HoverTarget::Tile { name, span } => {
            let tile = nf.and_then(|nf| nf.tiles.get(name));
            Some((render_tile(name, tile), *span))
        }
        HoverTarget::Partition { tile, name, span } => {
            let part = nf.and_then(|nf| nf.partitions.get(&(tile.clone(), name.clone())));
            Some((render_partition(tile, name, part), *span))
        }
        HoverTarget::Child { tile, partition, child_ref, span } => {
            let part = nf.and_then(|nf| nf.partitions.get(&(tile.clone(), partition.clone())));
            Some((render_child(tile, partition, child_ref, part), *span))
        }
    }
}

fn render_tile(name: &str, tile: Option<&NormalizedTile>) -> String {
    let mut out = format!("**tile** `{name}`\n");
    let Some(t) = tile else {
        out.push_str("\n*(not normalized)*");
        return out;
    };
    out.push_str(&format!("\n- symmetry: `{}`", fmt_symmetry(&t.symmetry)));
    if t.invariants.is_empty() {
        out.push_str("\n- affine invariants: `()`");
    } else {
        let inv: Vec<String> = t.invariants.iter().map(|v| format!("{v:.4}")).collect();
        out.push_str(&format!("\n- affine invariants: `({})`", inv.join(", ")));
    }
    if !t.group_perm.is_empty() {
        out.push_str("\n- group ↔ permutation:\n");
        out.push_str("```\n");
        for entry in &t.group_perm {
            out.push_str(&format!("  {:<10} → {:?}\n", entry.name, entry.perm));
        }
        out.push_str("```");
    }
    out
}

fn render_partition(tile: &str, name: &str, part: Option<&frac_lang::normalizer::NormalizedPartition>) -> String {
    let mut out = format!("**partition** `{tile}.{name}`\n");
    let Some(p) = part else {
        out.push_str("\n*(not normalized)*");
        return out;
    };
    out.push_str(&format!("\n- children: {}\n", p.children.len()));
    for c in &p.children {
        out.push_str(&format!(
            "  - **{}** `{}` (anchor v{}){}\n     polygon: {}\n     transform: {}\n",
            c.index,
            c.tile_type,
            c.anchor_vertex,
            c.name.as_ref().map(|n| format!(" `{n}`")).unwrap_or_default(),
            fmt_polygon(&c.polygon),
            fmt_transform(&c.similarity_transform),
        ));
    }
    out
}

fn render_child(
    tile: &str,
    partition: &str,
    child_ref: &ChildRefKind,
    part: Option<&frac_lang::normalizer::NormalizedPartition>,
) -> String {
    let label = match child_ref {
        ChildRefKind::Index(i) => format!("child {i}"),
        ChildRefKind::Name(n) => format!("child `{n}`"),
    };
    let mut out = format!("**{label}** in `{tile}.{partition}`\n");
    let Some(p) = part else {
        out.push_str("\n*(not normalized)*");
        return out;
    };
    let child: Option<&ChildInfo> = match child_ref {
        ChildRefKind::Index(i) => p.children.iter().find(|c| c.index == *i),
        ChildRefKind::Name(n) => p.children.iter().find(|c| c.name.as_deref() == Some(n.as_str())),
    };
    let Some(c) = child else {
        out.push_str("\n*(no matching child in normalized partition)*");
        return out;
    };
    out.push_str(&format!("\n- tile type: `{}`", c.tile_type));
    out.push_str(&format!("\n- canonical index: {}", c.index));
    if let Some(n) = &c.name {
        out.push_str(&format!("\n- name: `{n}`"));
    }
    out.push_str(&format!("\n- anchor vertex: {}", c.anchor_vertex));
    out.push_str(&format!("\n- polygon: {}", fmt_polygon(&c.polygon)));
    out.push_str(&format!("\n- similarity transform:\n```\n{}\n```", fmt_transform_block(&c.similarity_transform)));
    out
}

fn fmt_symmetry(g: &SymmetryGroup) -> String {
    match g {
        SymmetryGroup::Trivial => "trivial".to_string(),
        SymmetryGroup::Cyclic(n) => format!("C{n}"),
        SymmetryGroup::Dihedral(n) => format!("D{n}"),
    }
}

fn fmt_polygon(pts: &[[f64; 2]]) -> String {
    let parts: Vec<String> = pts.iter().map(|p| format!("({:.3},{:.3})", p[0], p[1])).collect();
    parts.join(" → ")
}

fn fmt_transform(t: &[f64; 6]) -> String {
    let det = t[0] * t[4] - t[1] * t[3];
    let orientation = if det < 0.0 { "(mirrored)" } else { "" };
    format!("scale≈{:.3}, det={:.3} {}", (det.abs()).sqrt(), det, orientation)
}

fn fmt_transform_block(t: &[f64; 6]) -> String {
    format!(
        "| {:>7.4}  {:>7.4}  {:>7.4} |\n| {:>7.4}  {:>7.4}  {:>7.4} |",
        t[0], t[1], t[2], t[3], t[4], t[5]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use frac_lang::parser::parse;

    #[test]
    fn finds_tile_decl_name() {
        let src = "tile quad {\n    sides 4\n    canonical (0,0) (1,0) (1,1) (0,1)\n}\n";
        let (file, _) = parse(src);
        // 'q' of "quad" is at offset 5
        let target = find_hover_target(&file, 5);
        assert!(matches!(target, Some(HoverTarget::Tile { ref name, .. }) if name == "quad"),
            "got {:?}", target);
    }

    #[test]
    fn finds_tile_ref_in_substitute() {
        let src = "tile q { sides 4 canonical (0,0) (1,0) (1,1) (0,1) }\n\
                   partition q.s { edge m=(0.5,0) cut m -- m }\n\
                   pattern { root q state{} color(0,0,0,1)\n\
                   rule q { -> q.s {} } }\n";
        let (file, _) = parse(src);
        let q_in_rule = src.find("rule q").unwrap() + 5;  // position of 'q' in `rule q`
        let target = find_hover_target(&file, q_in_rule);
        assert!(matches!(target, Some(HoverTarget::Tile { ref name, .. }) if name == "q"),
            "got {:?}", target);
    }
}
