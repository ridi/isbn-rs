use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use codegen::{Block, Function, Scope};
use roxmltree::{Document, Node};

const ALLOW_LINTS: &str = r###"
#[allow(clippy::unreadable_literal)]
#[allow(clippy::match_same_arms)]
#[allow(clippy::too_many_lines)]
"###;

/// EAN.UCC prefix or registration group.
struct Group {
    agency: String,
    prefix: [u8; 3],
    registration_group_element: Vec<u8>,
    rules: Vec<Rule>,
}

/// Range length rule.
struct Rule {
    min: u32,
    max: u32,
    length: usize,
}

/// Parse registration group and registrant range length rules.
fn parse_rules(group: Node) -> Vec<Rule> {
    group
        .descendants()
        .filter(|n| n.has_tag_name("Rule"))
        .map(|r| {
            let range: Vec<u32> = r
                .descendants()
                .find(|n| n.has_tag_name("Range"))
                .unwrap()
                .text()
                .unwrap()
                .split('-')
                .map(|i| i.parse().unwrap())
                .collect();

            let length = r
                .descendants()
                .find(|n| n.has_tag_name("Length"))
                .unwrap()
                .text()
                .unwrap()
                .parse()
                .unwrap();
            assert!(length < 8, "Segment length can be at most 7.");

            Rule {
                min: range[0],
                max: range[1],
                length,
            }
        })
        .collect()
}

/// Parse EAN.UCC prefix and registration group element.
fn parse_group(group: Node) -> Group {
    let prefix_str = group
        .descendants()
        .find(|n| n.has_tag_name("Prefix"))
        .unwrap()
        .text()
        .unwrap();

    let mut prefix = [0; 3];
    let mut registration_group_element = Vec::new();
    for (i, c) in prefix_str.chars().enumerate() {
        if i < 3 {
            prefix[i] = c.to_digit(10).unwrap() as u8;
        }
        if i >= 4 {
            registration_group_element.push(c.to_digit(10).unwrap() as u8)
        }
    }

    let agency = group
        .descendants()
        .find(|n| n.has_tag_name("Agency"))
        .unwrap()
        .text()
        .unwrap()
        .to_string();

    Group {
        agency,
        prefix,
        registration_group_element,
        rules: parse_rules(group),
    }
}

/// Generate code for EAN.UCC or registration group lookup.
fn codegen_find_group(name: &str, groups: Vec<Group>, check_registration_group: bool) -> Function {
    let mut fn_get_group = Function::new(name);
    fn_get_group.arg("prefix", "u16");

    if check_registration_group {
        fn_get_group.arg("registration_group_element", "u32");
    }

    fn_get_group.arg("segment", "u32");
    fn_get_group.ret("Result<Group<'static>, IsbnError>");

    let mut match_prefix = if check_registration_group {
        Block::new("match (prefix, registration_group_element)")
    } else {
        Block::new("match prefix")
    };

    for group in groups {
        match_prefix.line(if check_registration_group {
            format!(
                "({:#X}, {:#X}) =>",
                ((group.prefix[0] as u16) << 8)
                    | ((group.prefix[1] as u16) << 4)
                    | (group.prefix[2] as u16),
                {
                    let mut digits = 0u32;
                    for &digit in &group.registration_group_element {
                        digits = (digits << 4) | digit as u32;
                    }
                    digits
                }
            )
        } else {
            format!(
                "{:#X} =>",
                ((group.prefix[0] as u16) << 8)
                    | ((group.prefix[1] as u16) << 4)
                    | (group.prefix[2] as u16)
            )
        });

        let mut let_length_eq_match_segment = Block::new("let length = match segment");
        for rule in &group.rules {
            let_length_eq_match_segment.line(match rule.length {
                0 => format!(
                    "{} ..= {} => Err(IsbnError::UndefinedRange),",
                    rule.min, rule.max
                ),
                _ => format!("{} ..= {} => Ok({}),", rule.min, rule.max, rule.length),
            });
        }
        let_length_eq_match_segment.line("_ => Err(IsbnError::InvalidGroup)");
        let_length_eq_match_segment.after(";");

        let mut ok_group = Block::new("Ok(Group");
        ok_group.line(format!("name: \"{}\",", group.agency));
        ok_group.line("segment_length: length?");
        ok_group.after(")");

        let mut segment_match_block = Block::new("");
        segment_match_block.push_block(let_length_eq_match_segment);
        segment_match_block.push_block(ok_group);

        match_prefix.push_block(segment_match_block);
    }
    match_prefix.line("_ => Err(IsbnError::InvalidGroup)");
    fn_get_group.push_block(match_prefix);
    fn_get_group
}

fn main() {
    let mut f = File::open("./isbn-ranges/RangeMessage.xml").unwrap();
    let mut text = String::new();
    f.read_to_string(&mut text).unwrap();

    let range_message = Document::parse(&text).unwrap();
    let ean_ucc_groups = range_message
        .descendants()
        .filter(|d| d.tag_name().name() == "EAN.UCC")
        .map(parse_group)
        .collect();
    let registration_groups = range_message
        .descendants()
        .filter(|d| d.tag_name().name() == "Group")
        .map(parse_group)
        .collect();

    let mut scope = Scope::new();
    let impl_isbn = scope.new_impl("Isbn");
    impl_isbn.push_fn(codegen_find_group(
        "get_ean_ucc_group",
        ean_ucc_groups,
        false,
    ));
    impl_isbn.push_fn(codegen_find_group(
        "get_registration_group",
        registration_groups,
        true,
    ));

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated.rs");

    let mut f = File::create(&dest_path).unwrap();
    f.write_all(ALLOW_LINTS.trim_start().as_bytes()).unwrap();
    writeln!(f).unwrap();
    f.write_all(scope.to_string().as_bytes()).unwrap();
}
