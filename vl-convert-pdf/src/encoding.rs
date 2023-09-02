use lazy_static::lazy_static;
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Encoding {
    name: String,
    unicode_to_code: BTreeMap<char, u8>,
}

impl Encoding {
    /// The name of the encoding, as used in the font object.
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    /// Convert a Rust string to a vector of bytes in the encoding, converting
    /// unsupported characters to ASCII approximation
    pub fn encode_string(&self, text: &str) -> Vec<u8> {
        let size = text.len()
            + text
                .chars()
                .filter(|&c| c == '\\' || c == '(' || c == ')')
                .count();
        let mut result = Vec::with_capacity(size);
        for ch in text.chars() {
            match self.unicode_to_code.get(&ch).cloned() {
                Some(b'\\') => {
                    result.push(b'\\');
                    result.push(b'\\')
                }
                Some(b'(') => {
                    result.push(b'\\');
                    result.push(b'(')
                }
                Some(b')') => {
                    result.push(b'\\');
                    result.push(b')')
                }
                Some(ch) => result.push(ch),
                None => {
                    // When character is not found in the encoding, attempt to convert
                    // from unicode to ASCII characters (between 1-127)
                    if let Some(ch) = deunicode::deunicode_char(ch) {
                        result.extend(ch.as_bytes());
                    } else {
                        result.push(b'?');
                    }
                }
            }
        }
        result
    }
}

lazy_static! {
    pub static ref WIN_ANSI_ENCODING: Encoding = {
        let mut codes = BTreeMap::new();
        // See https://www.i18nqa.com/debug/table-iso8859-1-vs-windows-1252.html
        // > ISO-8859-1 (also called Latin-1) is identical to Windows-1252
        //   (also called CP1252) except for the code points 128-159 (0x80-0x9F).
        //   ISO-8859-1 assigns several control codes in this range. Windows-1252 has
        //   several characters, punctuation, arithmetic and business symbols assigned
        //   to these code points.

        // Initialize codes as matching ISO-8859-1
        for code in 32..255 {
            codes.insert(code as char, code);
        }

        // Overwrite Windows-1252 specific characters
        codes.insert('€', 128);
        codes.insert('‚', 130);
        codes.insert('ƒ', 131);
        codes.insert('„', 132);
        codes.insert('…', 133);
        codes.insert('†', 134);
        codes.insert('‡', 135);
        codes.insert('ˆ', 136);
        codes.insert('‰', 137);
        codes.insert('Š', 138);
        codes.insert('‹', 139);
        codes.insert('Œ', 140);
        codes.insert('Ž', 142);
        codes.insert('‘', 145);
        codes.insert('’', 146);
        codes.insert('“', 147);
        codes.insert('”', 148);
        codes.insert('•', 149);
        codes.insert('–', 150);
        codes.insert('—', 151);
        codes.insert('˜', 152);
        codes.insert('™', 153);
        codes.insert('š', 154);
        codes.insert('›', 155);
        codes.insert('ž', 158);
        codes.insert('Ÿ', 159);
        Encoding {
            name: "WinAnsiEncoding".to_string(),
            unicode_to_code: codes
        }
    };
}
