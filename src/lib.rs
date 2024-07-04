// Copyright 2013-2014 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! [*Unicode IDNA Compatibility Processing*
//! (Unicode Technical Standard #46)](http://www.unicode.org/reports/tr46/)

use self::Mapping::*;

include!("uts46_mapping_table.rs");

#[derive(Debug)]
struct StringTableSlice {
    // Store these as separate fields so the structure will have an
    // alignment of 1 and thus pack better into the Mapping enum, below.
    byte_start_lo: u8,
    byte_start_hi: u8,
    byte_len: u8,
}

fn decode_slice(slice: &StringTableSlice) -> &'static str {
    let lo = slice.byte_start_lo as usize;
    let hi = slice.byte_start_hi as usize;
    let start = (hi << 8) | lo;
    let len = slice.byte_len as usize;
    &STRING_TABLE[start..(start + len)]
}

#[repr(u8)]
#[derive(Debug)]
enum Mapping {
    Valid,
    Ignored,
    Mapped(StringTableSlice),
    Deviation(StringTableSlice),
    Disallowed,
    DisallowedStd3Valid,
    DisallowedStd3Mapped(StringTableSlice),
    DisallowedIdna2008,
}

fn find_char(codepoint: char) -> &'static Mapping {
    let idx = match TABLE.binary_search_by_key(&codepoint, |&val| val.0) {
        Ok(idx) => idx,
        Err(idx) => idx - 1,
    };

    const SINGLE_MARKER: u16 = 1 << 15;

    let (base, x) = TABLE[idx];
    let single = (x & SINGLE_MARKER) != 0;
    let offset = !SINGLE_MARKER & x;

    if single {
        &MAPPING_TABLE[offset as usize]
    } else {
        &MAPPING_TABLE[(offset + (codepoint as u16 - base as u16)) as usize]
    }
}

pub struct Mapper<I>
where
    I: Iterator<Item = char>,
{
    chars: I,
    slice: Option<core::str::Chars<'static>>,
    ignored_as_errors: bool,
}

impl<I> Mapper<I>
where
    I: Iterator<Item = char>,
{
    pub fn new(delegate: I, ignored_as_errors: bool) -> Self {
        Mapper {
            chars: delegate,
            slice: None,
            ignored_as_errors,
        }
    }
}

impl<I> Iterator for Mapper<I>
where
    I: Iterator<Item = char>,
{
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(s) = &mut self.slice {
                match s.next() {
                    Some(c) => return Some(c),
                    None => {
                        self.slice = None;
                    }
                }
            }

            let codepoint = self.chars.next()?;
            if let '.' | '-' | 'a'..='z' | '0'..='9' = codepoint {
                return Some(codepoint);
            }

            return Some(match *find_char(codepoint) {
                Mapping::Valid
                | Mapping::Deviation(_)
                | Mapping::DisallowedStd3Valid
                | Mapping::DisallowedIdna2008 => codepoint,
                Mapping::Ignored => {
                    if self.ignored_as_errors {
                        '\u{FFFD}'
                    } else {
                        continue;
                    }
                }
                Mapping::Mapped(ref slice) | Mapping::DisallowedStd3Mapped(ref slice) => {
                    self.slice = Some(decode_slice(slice).chars());
                    continue;
                }
                Mapping::Disallowed => '\u{FFFD}',
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{find_char, Mapping};

    #[test]
    fn mapping_fast_path() {
        assert_matches!(find_char('-'), &Mapping::Valid);
        assert_matches!(find_char('.'), &Mapping::Valid);
        for c in &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'] {
            assert_matches!(find_char(*c), &Mapping::Valid);
        }
        for c in &[
            'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q',
            'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
        ] {
            assert_matches!(find_char(*c), &Mapping::Valid);
        }
    }
}
