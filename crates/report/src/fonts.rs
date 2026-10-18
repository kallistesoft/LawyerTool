//! Helvetica / Helvetica-Bold (PDF standard fonts) with WinAnsi encoding:
//! widths from the Adobe AFM files, in 1/1000 em.

#[rustfmt::skip]
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // space .. /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, // 0 .. ?
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, // @ .. O
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, // P .. _
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, // ` .. o
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,      // p .. ~
];

#[rustfmt::skip]
const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611,
    975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556,
    333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611,
    611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

/// Unicode → WinAnsi (cp1252) byte; unknown characters become `?`.
pub fn winansi(c: char) -> u8 {
    match c {
        ' '..='~' => c as u8,
        '\u{a0}'..='\u{ff}' => c as u32 as u8,
        '€' => 0x80,
        '‚' => 0x82,
        '„' => 0x84,
        '…' => 0x85,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '–' => 0x96,
        '—' => 0x97,
        'œ' => 0x9c,
        'Œ' => 0x8c,
        '\u{202f}' | '\u{2009}' => b' ', // narrow / thin spaces (French typography)
        '✓' => b'v',
        '✗' => b'x',
        _ => b'?',
    }
}

/// The ASCII letter whose width an accented WinAnsi letter shares.
fn base(b: u8) -> u8 {
    match b {
        0xc0..=0xc5 => b'A',
        0xc7 => b'C',
        0xc8..=0xcb => b'E',
        0xcc..=0xcf => b'I',
        0xd1 => b'N',
        0xd2..=0xd6 | 0xd8 => b'O',
        0xd9..=0xdc => b'U',
        0xe0..=0xe5 => b'a',
        0xe7 => b'c',
        0xe8..=0xeb => b'e',
        0xec..=0xef => b'i',
        0xf1 => b'n',
        0xf2..=0xf6 | 0xf8 => b'o',
        0xf9..=0xfc => b'u',
        0xfd | 0xff => b'y',
        0x91 | 0x92 => b'\'',
        0x93 | 0x94 => b'"',
        0x96 => b'-',
        0xa0 => b' ',
        _ => b'n',
    }
}

/// Width of one WinAnsi byte, 1/1000 em.
pub fn width(b: u8, bold: bool) -> u16 {
    let table = if bold { &HELVETICA_BOLD } else { &HELVETICA };
    let ascii = if (32..=126).contains(&b) { b } else { base(b) };
    match b {
        0x97 => 1000, // em dash
        0x85 => 1000, // ellipsis
        0x9c | 0x8c => 944,
        _ => table[usize::from(ascii - 32)],
    }
}

/// Text width in points at `size`.
pub fn text_width(s: &str, size: f32, bold: bool) -> f32 {
    s.chars()
        .map(|c| f32::from(width(winansi(c), bold)))
        .sum::<f32>()
        * size
        / 1000.0
}
