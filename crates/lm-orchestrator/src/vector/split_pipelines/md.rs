use comrak::{Arena, Options, nodes::NodeValue, parse_document};
use serde::Serialize;
use crate::vector::{MIN_TOKENS, MAX_TOKENS};

#[derive(Debug, Serialize, Clone)]
struct Heading {
    depth: u32,
    title: String,
}

#[derive(Debug, Serialize, Clone)]
struct TreeNode {
    children: Vec<TreeNode>,
    content: Option<String>,
    heading: Heading,
}

#[derive(Debug, Serialize, Clone)]
struct PathElement {
    depth: u32,
    title: String,
}

struct Section {
    depth: u32,
    title: String,
    content: String,
}

fn parse_markdown_to_tree(markdown: &str, file_name: &str) -> TreeNode {
    let arena = Arena::new();

    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.front_matter_delimiter = Some("---".to_string());

    let root = parse_document(&arena, markdown, &options);

    let mut sections = vec![Section {
        depth: 0,
        title: file_name.to_string(),
        content: String::new(),
    }];
    let mut current_content = String::new();

    let flush_content = |content: &mut String, secs: &mut Vec<Section>| {
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() {
            if let Some(last) = secs.last_mut() {
                if !last.content.is_empty() {
                    last.content.push_str("\n\n");
                }
                last.content.push_str(&trimmed);
            } else {
                secs.push(Section {
                    depth: 0,
                    title: file_name.to_string(),
                    content: format!("# {}\n\n{}", file_name, trimmed),
                });
            }
        }
        content.clear();
    };

    for node in root.children() {
        let data = node.data.borrow();
        if let NodeValue::Heading(h) = &data.value {
            flush_content(&mut current_content, &mut sections);

            let mut buf = Vec::new();
            comrak::format_commonmark(node, &options, &mut buf)
                .expect("Failed to format heading to commonmark");
            let heading_text = String::from_utf8_lossy(&buf).trim().to_string();

            let title = heading_text
                .trim_start_matches(|c: char| c == '#' || c.is_whitespace())
                .trim()
                .to_string();

            sections.push(Section {
                depth: h.level as u32,
                title,
                content: heading_text.clone(),
            });
        } else {
            let mut buf = Vec::new();
            comrak::format_commonmark(node, &options, &mut buf)
                .expect("Failed to format node to commonmark");
            current_content.push_str(&String::from_utf8_lossy(&buf));
        }
    }

    flush_content(&mut current_content, &mut sections);

    if !sections[0].content.is_empty() {
        sections[0].content = format!("# {}\n\n{}", file_name, sections[0].content);
    }

    build_tree(&sections)
}

fn build_tree(sections: &[Section]) -> TreeNode {
    let first = &sections[0];
    let mut root = TreeNode {
        children: vec![],
        content: if first.content.is_empty() {
            None
        } else {
            Some(first.content.clone())
        },
        heading: Heading {
            depth: first.depth,
            title: first.title.clone(),
        },
    };

    let mut i = 1;
    while i < sections.len() {
        let sec = &sections[i];
        let mut child = TreeNode {
            children: vec![],
            content: if sec.content.is_empty() {
                None
            } else {
                Some(sec.content.clone())
            },
            heading: Heading {
                depth: sec.depth,
                title: sec.title.clone(),
            },
        };
        i = build_tree_recursive(sections, i + 1, sec.depth, &mut child.children);
        root.children.push(child);
    }
    root
}

fn build_tree_recursive(
    sections: &[Section],
    start_idx: usize,
    parent_depth: u32,
    children: &mut Vec<TreeNode>,
) -> usize {
    let mut i = start_idx;
    while i < sections.len() {
        let sec = &sections[i];
        if sec.depth <= parent_depth {
            break;
        }
        let mut child = TreeNode {
            children: vec![],
            content: if sec.content.is_empty() {
                None
            } else {
                Some(sec.content.clone())
            },
            heading: Heading {
                depth: sec.depth,
                title: sec.title.clone(),
            },
        };
        i = build_tree_recursive(sections, i + 1, sec.depth, &mut child.children);
        children.push(child);
    }
    i
}

fn flatten_and_chunk(
    node: &TreeNode,
    current_path: &mut Vec<PathElement>,
    chunks: &mut Vec<String>,
    count_tokens: &impl Fn(&str) -> usize,
) {
    current_path.push(PathElement {
        depth: node.heading.depth,
        title: node.heading.title.clone(),
    });

    if let Some(ref content) = node.content {
        if !content.trim().is_empty() {
            let mut prefix = String::new();
            for p in current_path.iter().take(current_path.len() - 1) {
                let level = if p.depth == 0 { 1 } else { p.depth as usize };
                let hashes = "#".repeat(level);
                prefix.push_str(&format!("{} {}\n\n", hashes, p.title));
            }

            let full_content = if prefix.is_empty() {
                content.clone()
            } else {
                format!("{}{}", prefix, content)
            };

            let slices = split_text_into_slices(&full_content, count_tokens);
            for slice in slices {
                chunks.push(slice);
            }
        }
    }

    for child in &node.children {
        flatten_and_chunk(child, current_path, chunks, count_tokens);
    }

    current_path.pop();
}

fn split_text_into_slices<F>(text: &str, count_tokens: &F) -> Vec<String>
where
    F: Fn(&str) -> usize,
{
    if text.is_empty() {
        return vec![];
    }

    if count_tokens(text) <= MAX_TOKENS {
        return vec![text.to_string()];
    }

    let mut slices = vec![];
    let mut current = String::new();

    for para in text.split("\n\n") {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }

        if count_tokens(para) > MAX_TOKENS {
            if count_tokens(&current) >= MIN_TOKENS {
                slices.push(std::mem::take(&mut current));
            }
            let sub_units = split_into_sub_units(para);
            process_sub_units(&sub_units, count_tokens, &mut current, &mut slices);
            continue;
        }

        if !try_append(para, "\n\n", count_tokens, &mut current, &mut slices) {
            let sub_units = split_into_sub_units(para);
            process_sub_units(&sub_units, count_tokens, &mut current, &mut slices);
        }
    }

    if !current.trim().is_empty() {
        slices.push(current.trim().to_string());
    }

    slices
}

fn try_append<F>(
    item: &str,
    separator: &str,
    count_tokens: &F,
    current: &mut String,
    slices: &mut Vec<String>,
) -> bool
where
    F: Fn(&str) -> usize,
{
    let candidate = if current.is_empty() {
        item.to_string()
    } else {
        format!("{}{}{}", current, separator, item)
    };

    if count_tokens(&candidate) <= MAX_TOKENS {
        *current = candidate;
        return true;
    }

    if count_tokens(current) >= MIN_TOKENS {
        slices.push(std::mem::take(current));
        *current = item.to_string();
        return true;
    }

    false
}

fn process_sub_units<F>(
    sub_units: &[String],
    count_tokens: &F,
    current: &mut String,
    slices: &mut Vec<String>,
) where
    F: Fn(&str) -> usize,
{
    for unit in sub_units {
        let unit = unit.trim();
        if unit.is_empty() {
            continue;
        }

        if !try_append(unit, "\n", count_tokens, current, slices) {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(unit);
        }
    }
}

fn split_into_sub_units(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let has_list = lines.iter().any(|l| {
        let t = l.trim();
        t.starts_with("* ")
            || t.starts_with("- ")
            || t.starts_with("+ ")
            || t.chars().next().map_or(false, |c| c.is_ascii_digit())
    });

    if has_list {
        return lines
            .into_iter()
            .map(|l| l.to_string())
            .filter(|s| !s.trim().is_empty())
            .collect();
    }

    text.split_inclusive(|c| c == '.' || c == '!' || c == '?')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn split_md_doc<F>(doc_text: &str, doc_name: &str, count_tokens: &F) -> Vec<String>
where
    F: Fn(&str) -> usize,
{
    let tree = parse_markdown_to_tree(&doc_text, &doc_name);
    let mut chunks = vec![];
    let mut path = vec![];

    flatten_and_chunk(&tree, &mut path, &mut chunks, &count_tokens);
    chunks
}
