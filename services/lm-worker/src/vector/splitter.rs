#![allow(dead_code)]

use crate::model::Document;

#[repr(usize)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FileFormat {
    Markdown = 0,
    Unsupported = 1,
    _Count = 2,
}

#[inline(always)]
pub fn map_file_format(format_str: &str) -> FileFormat {
    match format_str {
        "md" | "markdown" => FileFormat::Markdown,
        _ => FileFormat::Unsupported,
    }
}

pub type PipelineFn<F> = fn(&str, &str, &F) -> Vec<String>;

#[inline(always)]
fn unsupported_pipeline<F: Fn(&str) -> usize>(_: &str, _: &str, _: &F) -> Vec<String> {
    Vec::new()
}

pub struct DocSplitter<F: Fn(&str) -> usize> {
    pipelines: [PipelineFn<F>; FileFormat::_Count as usize],
}

impl<F: Fn(&str) -> usize> DocSplitter<F> {
    pub fn new() -> Self {
        Self {
            pipelines: [
                crate::vector::split_pipelines::md::split_md_doc::<F>,
                unsupported_pipeline::<F> as PipelineFn<F>,
            ],
        }
    }

    #[inline(always)]
    pub fn split(&self, doc: &Document, count_tokens: &F) -> Option<Vec<String>> {
        let idx = map_file_format(&doc.file_format) as usize;

        // SAFETY: idx <= num of supported files
        let pipeline = unsafe { *self.pipelines.get_unchecked(idx) };

        match pipeline(&doc.text, &doc.name, count_tokens) {
            chunks if !chunks.is_empty() => Some(chunks),
            _ => None
        }
    }
}
