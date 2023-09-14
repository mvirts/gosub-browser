use std::fs::File;
use std::io;
use std::io::Read;
use crate::html5_parser::tokenizer::{CHAR_CR, CHAR_LF};

// Encoding defines the way the buffer stream is read, as what defines a "character".
#[derive(PartialEq)]
pub enum Encoding {
    UTF8, // Stream is of UTF8 characters
    ASCII, // Stream is of 8bit ASCII
          // Iso88591        // Stream is of iso_8859_1
          // More
}

// The confidence decides how confident we are that the input stream is of this encoding
#[derive(PartialEq)]
pub enum Confidence {
    Tentative, // This encoding might be the one we need
    Certain,   // We are certain to use this encoding
               // Irrelevant          // There is no content encoding for this stream
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub struct Position {
    pub offset: usize,
    pub line: usize,
    pub col: usize,
}

impl Position {
    // Create a new position
    pub fn new(offset: usize, line: usize, col: usize) -> Self {
        Position {
            offset,
            line,
            col,
        }
    }

    // Returns a string representation of the position
    pub fn to_string(&self) -> String {
        format!("{}:{}:{}", self.offset, self.line, self.col)
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum Element {
    Utf8(char),             // Standard UTF character
    Surrogate(u16),         // Surrogate character (since they cannot be stored in <char>)
    Eof,                    // End of stream
}

impl Element {
    pub fn is_eof(&self) -> bool {
        match self {
            Element::Eof => true,
            _ => false,
        }
    }

    pub fn is_utf8(&self) -> bool {
        match self {
            Element::Utf8(_) => true,
            _ => false,
        }
    }

    pub fn is_surrogate(&self) -> bool {
        match self {
            Element::Surrogate(_) => true,
            _ => false,
        }
    }

    pub fn u32(&self) -> u32 {
        match self {
            Element::Utf8(c) => *c as u32,
            Element::Surrogate(c) => *c as u32,
            Element::Eof => 0,
        }
    }

    pub fn utf8(&self) -> char {
        match self {
            Element::Utf8(c) => *c,
            Element::Surrogate(..) => 0x0000 as char,
            Element::Eof => 0x0000 as char,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Element::Utf8(ch) => ch.to_string(),
            Element::Surrogate(surrogate) => format!("U+{:04X}", surrogate), // Or some other representation
            Element::Eof => "EOF".to_string(), // Or an empty string
        }
    }
}

// HTML(5) input stream structure
pub struct InputStream {
    pub encoding: Encoding,             // Current encoding
    pub confidence: Confidence,         // How confident are we that this is the correct encoding?

    pub position: Position,             // Current positions
    pub length: usize,                  // Length (in chars) of the buffer
    line_offsets: Vec<usize>,           // Offsets of the given lines

    buffer: Vec<Element>,               // Reference to the actual buffer stream in characters
    u8_buffer: Vec<u8>,                 // Reference to the actual buffer stream in u8 bytes
                                        // If all things are ok, both buffer and u8_buffer should refer to the same memory location (?)

    pub has_read_eof: bool,             // True when we just read an EOF
}

pub enum SeekMode {
    SeekSet,       // Seek from the start of the stream
    SeekCur,       // Seek from the current stream position
    SeekEnd,       // Seek (backwards) from the end of the stream
}

impl InputStream {
    // Create a new default empty input stream
    pub fn new() -> Self {
        InputStream {
            encoding: Encoding::UTF8,
            confidence: Confidence::Tentative,
            position: Position{
                offset: 0,
                line: 1,
                col: 1,
            },
            length: 0,
            line_offsets: vec![0],      // first line always starts at 0
            buffer: Vec::new(),
            u8_buffer: Vec::new(),
            has_read_eof: false,
        }
    }

    // Returns true when the encoding encountered is defined as certain
    pub fn is_certain_encoding(&self) -> bool {
        self.confidence == Confidence::Certain
    }

    // Detect the given encoding from stream analysis
    pub fn detect_encoding(&self) {
        todo!()
    }

    // Returns true when the stream pointer is at the end of the stream
    pub fn eof(&self) -> bool {
        self.has_read_eof || self.position.offset >= self.length
    }

    // Reset the stream reader back to the start
    pub fn reset(&mut self) {
        self.position.offset = 0;
        self.position.line = 1;
        self.position.col = 1;
    }

    // Seek explicit offset in the stream (based on chars)
    pub fn seek(&mut self, mode: SeekMode, offset: isize) {
        let abs_offset = match mode {
            SeekMode::SeekSet => {
                if offset.is_negative() {
                    0
                } else {
                    offset as usize
                }
            }
            SeekMode::SeekCur => {
                if offset.is_negative() {
                    self.position.offset - offset.abs() as usize
                } else {
                    self.position.offset + offset as usize
                }
            }
            SeekMode::SeekEnd => {
                // Both -5 and 5 on seek-end do the same thing
                if offset.abs() > self.length as isize {
                    0
                } else {
                    self.length - offset.abs() as usize
                }
            }
        };

        self.position = self.generate_position(abs_offset);
    }

    pub fn get_previous_position(&mut self) -> Position {

        // if we are at the begining or the end of the stream, we just return the current position
        if self.position.offset == 0 || self.has_read_eof {
            return self.position;
        }

        self.generate_position(self.position.offset - 1)
    }

    // Generate a new position structure for given offset
    fn generate_position(&mut self, abs_offset: usize) -> Position {
        let mut abs_offset = abs_offset;

        // Cap to length if we read past the end of the stream
        if abs_offset > self.length + 1  {
            abs_offset = self.length;
            self.has_read_eof = true;
        }

        // Detect lines (if needed)
        self.read_line_endings_until(abs_offset);

        let mut last_line: usize = 0;
        let mut last_offset = self.line_offsets[last_line];
        for i in 0..self.line_offsets.len() {
            if self.line_offsets[i] > abs_offset {
                break;
            }

            last_line = i;
            last_offset = self.line_offsets[last_line];
        }

        // Set position values
        return Position{
            offset: abs_offset,
            line: last_line + 1,
            col: abs_offset - last_offset + 1,
        }
    }

    pub fn tell(&self) -> usize {
        self.position.offset
    }

    // Set the given confidence of the input stream encoding
    pub fn set_confidence(&mut self, c: Confidence) {
        self.confidence = c;
    }

    // Changes the encoding and if necessary, decodes the u8 buffer into the correct encoding
    pub fn set_encoding(&mut self, e: Encoding) {
        // Don't convert if the encoding is the same as it already is
        if self.encoding == e {
            return;
        }

        self.force_set_encoding(e)
    }

    // Sets the encoding for this stream, and decodes the u8_buffer into the buffer with the
    // correct encoding.
    pub fn force_set_encoding(&mut self, e: Encoding) {
        match e {
            Encoding::UTF8 => {
                let str_buf;
                unsafe {
                    str_buf = std::str::from_utf8_unchecked(&self.u8_buffer)
                        .replace("\u{000D}\u{000A}", "\u{000A}")
                        .replace("\u{000D}", "\u{000A}");
                }

                // Convert the utf8 string into characters so we can use easy indexing
                self.buffer = vec![];
                for c in str_buf.chars() {

                    // // Check if we have a non-bmp character. This means it's above 0x10000
                    // let cp = c as u32;
                    // if cp > 0x10000 && cp <= 0x10FFFF {
                    //     let adjusted = cp - 0x10000;
                    //     let lead = ((adjusted >> 10) & 0x3FF) as u16 + 0xD800;
                    //     let trail = (adjusted & 0x3FF) as u16 + 0xDC00;
                    //     self.buffer.push(Element::Surrogate(lead));
                    //     self.buffer.push(Element::Surrogate(trail));
                    //     continue;
                    // }

                    if (0xD800..=0xDFFF).contains(&(c as u32)) {
                        self.buffer.push(Element::Surrogate(c as u16));
                    } else {
                        self.buffer.push(Element::Utf8(c));
                    }
                }
                self.length = self.buffer.len();
            }
            Encoding::ASCII => {
                // Convert the string into characters so we can use easy indexing. Any non-ascii chars (> 0x7F) are converted to '?'
                self.buffer = self.normalize_newlines_and_ascii(&self.u8_buffer);
                self.length = self.buffer.len();
            }
        }

        self.encoding = e;
    }

    fn normalize_newlines_and_ascii(&self, buffer: &Vec<u8>) -> Vec<Element> {
        let mut result = Vec::with_capacity(buffer.len());

        for i in 0..buffer.len() {
            if buffer[i] == CHAR_CR as u8 {
                // convert CR to LF, or CRLF to LF
                if i + 1 < buffer.len() && buffer[i + 1] == CHAR_LF as u8 {
                    continue;
                }
                result.push(Element::Utf8(CHAR_LF));
            } else if buffer[i] >= 0x80 {
                // Convert high ascii to ?
                result.push(Element::Utf8('?'));
            } else {
                // everything else is ok
                result.push(Element::Utf8(buffer[i] as char))
            }
        }

        return result
    }

    // Populates the current buffer with the contents of given file f
    pub fn read_from_file(&mut self, mut f: File, e: Option<Encoding>) -> io::Result<()> {
        // First we read the u8 bytes into a buffer
        f.read_to_end(&mut self.u8_buffer).expect("uh oh");
        self.force_set_encoding(e.unwrap_or(Encoding::UTF8));
        self.reset();
        Ok(())
    }

    // Populates the current buffer with the contents of the given string s
    pub fn read_from_str(&mut self, s: &str, e: Option<Encoding>) {
        self.u8_buffer = Vec::from(s.as_bytes());
        self.force_set_encoding(e.unwrap_or(Encoding::UTF8));
        self.reset();
    }

    // Returns the number of characters left in the buffer
    pub(crate) fn chars_left(&self) -> usize {
        self.length - self.position.offset
    }

    // Reads a character and increases the current pointer, or read EOF as None
    pub(crate) fn read_char(&mut self) -> Element {
        // Return none if we already have read EOF
        if self.has_read_eof {
            return Element::Eof;
        }

        // If we still can move forward in the stream, move forwards
        return if self.position.offset < self.length {
            let c = self.buffer[self.position.offset].clone();
            self.seek(SeekMode::SeekCur, 1);
            c
        } else {
            // otherwise, we have reached the end of the stream
            self.has_read_eof = true;

            self.seek(SeekMode::SeekEnd, 0);

            // // This is a kind of dummy position so the end of the files are read correctly.
            // self.position = Position{
            //     offset: self.position.offset,
            //     line: self.position.line,
            //     col: self.position.col,
            // };

            Element::Eof
        }
    }

    pub(crate) fn unread(&mut self) {
        // We already read eof, so "unread" the eof by unsetting the flag
        if self.has_read_eof {
            self.has_read_eof = false;
            return;
        }

        // If we can track back from the offset, we can do so
        if self.position.offset > 0 {
            self.seek(SeekMode::SeekCur, -1);
        }
    }

    // Looks ahead in the stream and returns len characters
    pub(crate) fn look_ahead_slice(&self, len: usize) -> String {
        let end_pos = std::cmp::min(self.length, self.position.offset + len);

        let slice = &self.buffer[self.position.offset..end_pos];
        slice.iter().map(|e| e.to_string()).collect()
    }

    // Looks ahead in the stream, can use an optional index if we want to seek further
    // (or back) in the stream.
    pub(crate) fn look_ahead(&self, offset: usize) -> Element {
        // Trying to look after the stream
        if self.position.offset + offset >= self.length {
            return Element::Eof;
        }

        self.buffer[self.position.offset + offset]
    }

    // Populates the line endings
    fn read_line_endings_until(&mut self, abs_offset: usize) {
        let mut last_offset = *self.line_offsets.last().unwrap();

        while last_offset <= abs_offset {
            if last_offset >= self.length {
                self.line_offsets.push(last_offset + 1);
                break;
            }

            // Check the next char to see if it's a '\n'
            let c = self.buffer[last_offset].clone();
            if c == Element::Utf8('\n') {
                self.line_offsets.push(last_offset + 1);
            }

            last_offset += 1;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_stream() {
        let mut is = InputStream::new();
        assert_eq!(is.eof(), true);

        is.read_from_str("foo", Some(Encoding::ASCII));
        assert_eq!(is.length, 3);
        assert_eq!(is.eof(), false);
        assert_eq!(is.chars_left(), 3);

        is.read_from_str("f👽f", Some(Encoding::UTF8));
        assert_eq!(is.length, 3);
        assert_eq!(is.eof(), false);
        assert_eq!(is.chars_left(), 3);
        assert_eq!(is.read_char().utf8(), 'f');
        assert_eq!(is.chars_left(), 2);
        assert_eq!(is.eof(), false);
        assert_eq!(is.read_char().utf8(), '👽');
        assert_eq!(is.eof(), false);
        assert_eq!(is.chars_left(), 1);
        assert_eq!(is.read_char().utf8(), 'f');
        assert_eq!(is.eof(), true);
        assert_eq!(is.chars_left(), 0);

        is.reset();
        is.set_encoding(Encoding::ASCII);
        assert_eq!(is.length, 6);
        assert_eq!(is.read_char().utf8(), 'f');
        assert_eq!(is.read_char().utf8(), '?');
        assert_eq!(is.read_char().utf8(), '?');
        assert_eq!(is.read_char().utf8(), '?');
        assert_eq!(is.read_char().utf8(), '?');
        assert_eq!(is.read_char().utf8(), 'f');
        assert_eq!(is.read_char().is_eof(), true);

        is.unread();    // unread eof
        is.unread();    // unread 'f'
        is.unread();    // Unread '?'
        assert_eq!(is.chars_left(), 2);
        is.unread();
        assert_eq!(is.chars_left(), 3);

        is.reset();
        assert_eq!(is.chars_left(), 6);
        is.unread();
        assert_eq!(is.chars_left(), 6);


        is.read_from_str("abc", Some(Encoding::UTF8));
        is.reset();
        assert_eq!(is.read_char().utf8(), 'a');
        is.unread();
        assert_eq!(is.read_char().utf8(), 'a');
        assert_eq!(is.read_char().utf8(), 'b');
        is.unread();
        assert_eq!(is.read_char().utf8(), 'b');
        assert_eq!(is.read_char().utf8(), 'c');
        is.unread();
        assert_eq!(is.read_char().utf8(), 'c');
        assert_eq!(is.read_char().is_eof(), true);
        is.unread();
        assert_eq!(is.read_char().is_eof(), true);
    }

    #[test]
    fn test_certainty() {
        let mut is = InputStream::new();
        assert_eq!(is.is_certain_encoding(), false);

        is.set_confidence(Confidence::Certain);
        assert_eq!(is.is_certain_encoding(), true);

        is.set_confidence(Confidence::Tentative);
        assert_eq!(is.is_certain_encoding(), false);
    }

    #[test]
    fn test_offsets() {
        let mut is = InputStream::new();
        is.read_from_str("abc", Some(Encoding::UTF8));
        assert_eq!(is.position, Position{ offset: 0, line: 1, col: 1});
        assert_eq!('a', is.read_char().utf8());
        assert_eq!(is.position, Position{ offset: 1, line: 1, col: 2});
        assert_eq!('b', is.read_char().utf8());
        assert_eq!(is.position, Position{ offset: 2, line: 1, col: 3});
        assert_eq!('c', is.read_char().utf8());
        assert_eq!(is.position, Position{ offset: 3, line: 1, col: 4});
        assert_eq!(is.read_char().is_eof(), true);
        assert_eq!(is.position, Position{ offset: 3, line: 1, col: 4});
        assert_eq!(is.read_char().is_eof(), true);
        assert_eq!(is.position, Position{ offset: 3, line: 1, col: 4});


        let mut is = InputStream::new();
        is.read_from_str("abc\ndefg\n\nhi\njk\nlmno\n\n\npqrst\nu\nv\nw\n\nxy\nz", Some(Encoding::UTF8));
        assert_eq!(is.length, 40);

        is.seek(SeekMode::SeekSet, 0);
        assert_eq!(is.position, Position{ offset: 0, line: 1, col: 1});
        let c = is.read_char();
        assert_eq!('a', c.utf8());
        assert_eq!(is.position, Position{ offset: 1, line: 1, col: 2});

        is.seek(SeekMode::SeekSet, 7);
        assert_eq!(is.position, Position{ offset: 7, line: 2, col: 4});
        assert_eq!(is.chars_left(), 33);

        let c = is.read_char();
        assert_eq!('g', c.utf8());
        assert_eq!(is.position, Position{ offset: 8, line: 2, col: 5});

        let c = is.read_char();
        assert_eq!('\n', c.utf8());
        assert_eq!(is.position, Position{ offset: 9, line: 3, col: 1});

        let c = is.read_char();
        assert_eq!('\n', c.utf8());
        assert_eq!(is.position, Position{ offset: 10, line: 4, col: 1});

        let c = is.read_char();
        assert_eq!('h', c.utf8());
        assert_eq!(is.position, Position{ offset: 11, line: 4, col: 2});
        assert_eq!(is.chars_left(), 29);

        is.reset();
        assert_eq!(is.position, Position{ offset: 0, line: 1, col: 1});
        assert_eq!(is.chars_left(), 40);

        is.seek(SeekMode::SeekSet, 100);
        assert_eq!(is.position, Position{ offset: 40, line: 15, col: 2});
        assert_eq!(is.chars_left(), 0);
    }

    #[test]
    fn test_seek() {
        let mut is = InputStream::new();
        is.read_from_str("ab👽cd", Some(Encoding::UTF8));
        assert_eq!(is.length, 5);
        assert_eq!(is.chars_left(), 5);
        assert_eq!(is.read_char().utf8(), 'a');
        assert_eq!(is.read_char().utf8(), 'b');
        assert_eq!(is.chars_left(), 3);
        is.seek(SeekMode::SeekSet, 0);
        assert_eq!(is.chars_left(), 5);
        assert_eq!(is.read_char().utf8(), 'a');
        assert_eq!(is.read_char().utf8(), 'b');
        assert_eq!(is.chars_left(), 3);
        is.seek(SeekMode::SeekSet, 3);
        assert_eq!(is.chars_left(), 2);
        assert_eq!(is.read_char().utf8(), 'c');
        assert_eq!(is.read_char().utf8(), 'd');
        assert_eq!(is.chars_left(), 0);
        assert_eq!(is.eof(), true);

        is.reset();
        assert_eq!(is.look_ahead(0).utf8(), 'a');
        assert_eq!(is.look_ahead(3).utf8(), 'c');
        assert_eq!(is.look_ahead(1).utf8(), 'b');
        assert_eq!(is.look_ahead(100).is_eof(), true);

        is.seek(SeekMode::SeekSet, 0);
        assert_eq!(is.look_ahead_slice(1), "a");
        assert_eq!(is.look_ahead_slice(2), "ab");
        assert_eq!(is.look_ahead_slice(3), "ab👽");
        assert_eq!(is.look_ahead_slice(4), "ab👽c");
        assert_eq!(is.look_ahead_slice(5), "ab👽cd");
        assert_eq!(is.look_ahead_slice(6), "ab👽cd");
        assert_eq!(is.look_ahead_slice(100), "ab👽cd");

        is.seek(SeekMode::SeekSet, 3);
        assert_eq!(is.look_ahead_slice(1), "c");
        assert_eq!(is.look_ahead_slice(2), "cd");


        is.seek(SeekMode::SeekSet, 0);
        assert_eq!(is.position.offset, 0);

        is.seek(SeekMode::SeekSet, 3);
        assert_eq!(is.position.offset, 3);

        is.seek(SeekMode::SeekCur, 0);
        assert_eq!(is.position.offset, 3);

        is.seek(SeekMode::SeekCur, 1);
        assert_eq!(is.position.offset, 4);

        is.seek(SeekMode::SeekCur, -2);
        assert_eq!(is.position.offset, 2);

        is.seek(SeekMode::SeekCur, 10);
        assert_eq!(is.position.offset, 5);

        is.seek(SeekMode::SeekSet, 100);
        assert_eq!(is.position.offset, 5);

        is.seek(SeekMode::SeekSet, -100);
        assert_eq!(is.position.offset, 0);

        is.seek(SeekMode::SeekEnd, -100);
        assert_eq!(is.position.offset, 0);
    }

    #[test]
    fn test_eof() {
        let mut is = InputStream::new();
        is.read_from_str("abc", Some(Encoding::UTF8));
        assert_eq!(is.length, 3);
        assert_eq!(is.chars_left(), 3);
        assert_eq!(is.read_char().utf8(), 'a');
        assert_eq!(is.read_char().utf8(), 'b');
        assert_eq!(is.read_char().utf8(), 'c');
        assert_eq!(is.read_char().is_eof(), true);
        assert_eq!(is.read_char().is_eof(), true);
        assert_eq!(is.read_char().is_eof(), true);
        assert_eq!(is.read_char().is_eof(), true);
        is.unread();
        assert_eq!(is.read_char().is_eof(), true);
        is.unread();
        is.unread();
        assert_eq!(is.read_char().is_eof(), false);
        assert_eq!(is.read_char().is_eof(), true);
        is.unread();
        is.unread();
        assert_eq!(is.read_char().is_eof(), false);
        is.unread();
        is.unread();
        is.unread();
        assert_eq!(is.read_char().utf8(), 'a');
        is.unread();
        assert_eq!(is.read_char().utf8(), 'a');
        is.unread();
        is.unread();
        assert_eq!(is.read_char().utf8(), 'a');
        is.unread();
        is.unread();
        is.unread();
        is.unread();
        is.unread();
        is.unread();
        assert_eq!(is.read_char().utf8(), 'a');
        assert_eq!(is.read_char().utf8(), 'b');
        assert_eq!(is.read_char().utf8(), 'c');
        assert_eq!(is.read_char().is_eof(), true);
        is.unread();
        is.unread();
        assert_eq!(is.read_char().utf8(), 'c');
        assert_eq!(is.read_char().is_eof(), true);
        is.unread();
        assert_eq!(is.read_char().is_eof(), true);
    }
}
