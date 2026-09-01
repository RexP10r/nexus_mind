use crate::{model::Document, vector::split_pipelines::md::split_md_doc};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FileFormat {
    Markdown,
    Unsupported,
}

pub fn map_file_format(format_str: &str) -> FileFormat {
    match format_str {
        "md" | "markdown" => FileFormat::Markdown,
        _ => FileFormat::Unsupported,
    }
}

pub fn split_doc<F: Fn(&str) -> usize>(doc: &Document, count_tokens: &F) -> Option<Vec<String>> {
    match map_file_format(&doc.file_format) {
        FileFormat::Markdown => Some(split_md_doc(&doc.text, &doc.name, &count_tokens)),
        _ => None,
    }
}
