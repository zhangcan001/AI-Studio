//! UTF-8 source locations shared by every ScriptSource parser.

use crate::domain::script_draft::SourceSpan;
use std::{error::Error, fmt};

#[derive(Clone)]
pub struct SourceMap {
    raw: Vec<u8>,
    text: String,
    lines: Vec<SourceLine>,
    bom_len: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLine {
    pub number: u32,
    pub start_byte: usize,
    pub content_end_byte: usize,
    pub end_byte: usize,
    text: String,
}

impl SourceLine {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn text_for_range(&self, start_byte: usize, end_byte: usize) -> &str {
        let start = start_byte
            .saturating_sub(self.start_byte)
            .min(self.text.len());
        let end = end_byte
            .saturating_sub(self.start_byte)
            .min(self.text.len())
            .max(start);
        self.text.get(start..end).unwrap_or_default()
    }

    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceMapError {
    InvalidUtf8,
}

impl fmt::Display for SourceMapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("INVALID_SOURCE_UTF8")
    }
}

impl Error for SourceMapError {}

impl SourceMap {
    pub fn new(raw: &[u8]) -> Result<Self, SourceMapError> {
        let decoded = std::str::from_utf8(raw).map_err(|_| SourceMapError::InvalidUtf8)?;
        let bom_len = if raw.starts_with(b"\xef\xbb\xbf") {
            3
        } else {
            0
        };
        let mut lines = Vec::new();
        let mut line_start = bom_len;
        let mut line_number = 0u32;
        let mut cursor = bom_len;

        while cursor < raw.len() {
            if raw[cursor] == b'\n' {
                let content_end = if cursor > line_start && raw[cursor - 1] == b'\r' {
                    cursor - 1
                } else {
                    cursor
                };
                lines.push(SourceLine {
                    number: line_number,
                    start_byte: line_start,
                    content_end_byte: content_end,
                    end_byte: cursor + 1,
                    text: std::str::from_utf8(&raw[line_start..content_end])
                        .map_err(|_| SourceMapError::InvalidUtf8)?
                        .to_owned(),
                });
                line_number += 1;
                cursor += 1;
                line_start = cursor;
            } else {
                cursor += 1;
            }
        }

        let content_end = raw.len();
        if line_start <= raw.len() {
            lines.push(SourceLine {
                number: line_number,
                start_byte: line_start,
                content_end_byte: content_end,
                end_byte: raw.len(),
                text: std::str::from_utf8(&raw[line_start..content_end])
                    .map_err(|_| SourceMapError::InvalidUtf8)?
                    .to_owned(),
            });
        }

        Ok(Self {
            raw: raw.to_vec(),
            text: decoded
                .strip_prefix('\u{feff}')
                .unwrap_or(decoded)
                .to_owned(),
            lines,
            bom_len,
        })
    }

    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Returns the semantic UTF-8 text with a leading UTF-8 BOM removed.
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn lines(&self) -> &[SourceLine] {
        &self.lines
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn bom_len(&self) -> usize {
        self.bom_len
    }

    pub fn has_bom(&self) -> bool {
        self.bom_len != 0
    }

    /// Convert an original-byte range into a 0-based, end-exclusive span.
    /// Character positions are Unicode scalar-value indexes, never UTF-8 or
    /// UTF-16 indexes.
    pub fn span(&self, start_byte: usize, end_byte: usize) -> SourceSpan {
        let start = self.position(start_byte.min(self.raw.len()));
        let end = self.position(end_byte.min(self.raw.len()));
        SourceSpan::with_positions(
            start_byte.min(self.raw.len()) as u64,
            end_byte.min(self.raw.len()) as u64,
            start.0,
            end.0,
            start.1,
            end.1,
        )
    }

    pub fn position(&self, byte_offset: usize) -> (u32, u32) {
        if self.lines.is_empty() {
            return (0, 0);
        }
        let offset = byte_offset.min(self.raw.len());
        let line = self
            .lines
            .iter()
            .enumerate()
            .find(|(_, line)| offset < line.end_byte || line.end_byte == self.raw.len())
            .map(|(index, line)| (index, line))
            .unwrap_or_else(|| (self.lines.len().saturating_sub(1), &self.lines[0]));

        let character_end = line.1.content_end_byte;
        let character_offset = offset.clamp(line.1.start_byte, character_end);
        let character = std::str::from_utf8(&self.raw[line.1.start_byte..character_offset])
            .map(|text| text.chars().count() as u32)
            .unwrap_or(0);
        (line.1.number, character)
    }

    pub fn line_for_byte(&self, byte_offset: usize) -> Option<&SourceLine> {
        let offset = byte_offset.min(self.raw.len());
        self.lines
            .iter()
            .find(|line| offset < line.end_byte || line.end_byte == self.raw.len())
    }
}

#[cfg(test)]
mod tests {
    use super::SourceMap;

    #[test]
    fn maps_bom_crlf_and_unicode_scalar_positions() {
        let raw = "\u{feff}甲😊\r\n乙".as_bytes();
        let map = SourceMap::new(raw).unwrap();
        assert_eq!(map.bom_len(), 3);
        assert_eq!(map.lines()[0].text(), "甲😊");
        assert_eq!(map.lines()[0].start_byte, 3);
        assert_eq!(map.lines()[0].content_end_byte, 10);
        assert_eq!(map.span(3, 10).start_character, 0);
        assert_eq!(map.span(3, 10).end_character, 2);
        assert_eq!(map.span(3, 10).end_line, 0);
        assert_eq!(map.span(12, 15).start_line, 1);
        assert_eq!(map.span(12, 15).end_character, 1);
    }

    #[test]
    fn keeps_lf_offsets_and_empty_trailing_line() {
        let map = SourceMap::new(b"a\nb\n").unwrap();
        assert_eq!(map.lines().len(), 3);
        assert_eq!(map.lines()[1].start_byte, 2);
        assert_eq!(map.lines()[2].start_byte, 4);
        assert_eq!(map.span(0, 3).end_line, 1);
    }

    #[test]
    fn bom_unicode_lines_use_raw_byte_boundaries_for_every_line() {
        let raw = "\u{feff}😀\n汉字\r\n尾".as_bytes();
        let map = SourceMap::new(raw).unwrap();
        assert_eq!(map.lines()[0].text(), "😀");
        assert_eq!(map.lines()[1].text(), "汉字");
        assert_eq!(map.lines()[2].text(), "尾");
        assert_eq!(map.lines()[1].start_byte, 8);
        assert_eq!(map.lines()[1].content_end_byte, 14);
        assert_eq!(map.span(8, 14).start_character, 0);
        assert_eq!(map.span(8, 14).end_character, 2);
    }

    #[test]
    fn rejects_invalid_utf8_without_exposing_bytes() {
        assert!(SourceMap::new(&[0xff]).is_err());
    }
}
