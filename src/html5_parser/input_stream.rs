use std::fs::File;
use std::io;
use std::io::Read;

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

// HTML(5) input stream structure
pub struct InputStream {
    encoding: Encoding,                 // Current encoding
    pub(crate) confidence: Confidence,  // How confident are we that this is the correct encoding?
    pub current_offset: usize,              // Current offset of the reader
    pub current_line: usize,                // Current line (1 based)
    pub current_line_offset: usize,         // Current offset on line (1 based)
    pub length: usize,                      // Length (in chars) of the buffer
    buffer: Vec<char>,                  // Reference to the actual buffer stream in characters
    u8_buffer: Vec<u8>,                 // Reference to the actual buffer stream in u8 bytes
                                        // If all things are ok, both buffer and u8_buffer should refer to the same memory location
    line_offset: Vec<usize>,            // Offsets for all lines (or at least the lines already scanned)
}

impl InputStream {
    // Create a new default empty input stream
    pub fn new() -> Self {
        InputStream {
            encoding: Encoding::UTF8,
            confidence: Confidence::Tentative,
            current_offset: 0,
            current_line_offset: 1,
            current_line: 1,
            length: 0,
            buffer: Vec::new(),
            u8_buffer: Vec::new(),
            line_offset: vec![0],       // Line 0 (actually 1) starts at offset 0
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
        self.current_offset >= self.length
    }

    // Reset the stream reader back to the start
    pub fn reset(&mut self) {
        self.current_offset = 0;
        self.current_line = 1;
        self.current_line_offset = 1;
    }

    // Seek explicit offset in the stream (based on chars)
    pub fn seek(&mut self, mut off: usize) {
        // cap offset to the length of the stream
        if off > self.length {
            off = self.length - 1
        }

        // Try and find the line number for this offset
        let mut last_offset = 0;
        let mut ln = 1;
        for o in self.line_offset.clone() {
            // Found the line
            if o > off {
                self.current_offset = off;
                self.current_line = ln;
                self.current_line_offset = off - last_offset + 1;
                return;
            }

            last_offset = o;
            ln += 1;
        }

        // No offset found. We haven't scanned this far in the stream yet, scan further until we find offset
        loop {
            self.current_offset += 1;
            self.current_line_offset += 1;

            // Check the next char to see if it's a '\n'
            let c = self.buffer[self.current_offset];
            if c == '\n' {
                self.line_offset.push(self.current_offset);
                self.current_line += 1;
                self.current_line_offset = 0;
            }

            // End of stream or found our offset
            if self.current_offset >= self.length || self.current_offset == off {
                break;
            }
        }
    }

    pub fn tell(&self) -> usize {
        self.current_offset
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
                // Convert the u8 buffer into utf8 string
                let str_buf = std::str::from_utf8(&self.u8_buffer).unwrap();

                // Convert the utf8 string into characters so we can use easy indexing
                self.buffer = str_buf.chars().collect();
                self.length = self.buffer.len();
            }
            Encoding::ASCII => {
                // Convert the string into characters so we can use easy indexing. Any non-ascii chars (> 0x7F) are converted to '?'
                self.buffer = self
                    .u8_buffer
                    .iter()
                    .map(|&byte| if byte.is_ascii() { byte as char } else { '?' })
                    .collect();
                self.length = self.buffer.len();
            }
        }

        self.encoding = e;
    }

    // Populates the current buffer with the contents of given file f
    pub fn read_from_file(&mut self, mut f: File, e: Option<Encoding>) -> io::Result<()> {
        // First we read the u8 bytes into a buffer
        f.read_to_end(&mut self.u8_buffer).expect("uh oh");
        self.force_set_encoding(e.unwrap_or(Encoding::UTF8));
        self.current_offset = 0;
        Ok(())
    }

    // Populates the current buffer with the contents of the given string s
    pub fn read_from_str(&mut self, s: &str, e: Option<Encoding>) {
        self.u8_buffer = Vec::from(s.as_bytes());
        self.force_set_encoding(e.unwrap_or(Encoding::UTF8));
        self.current_offset = 0;
    }

    // Returns the number of characters left in the buffer
    pub(crate) fn chars_left(&self) -> usize {
        self.length - self.current_offset
    }

    // Reads a character and increases the current pointer
    pub(crate) fn read_char(&mut self) -> Option<char> {
        if self.eof() {
            return None;
        }

        let c = self.buffer[self.current_offset];
        self.current_offset += 1;
        self.current_line_offset += 1;

        if c == '\n' {
            self.current_line += 1;
            self.current_line_offset = 1;
        }

        return Some(c);
    }

    pub(crate) fn unread(&mut self) {
        if self.current_offset > 1 {
            self.current_offset -= 1;
        }
    }

    // Looks ahead in the stream, can use an optional index if we want to seek further
    // (or back) in the stream.
    // @TODO: idx can be pos or neg. But self.current_offset is always positive. This clashes.
    pub(crate) fn look_ahead(&self, idx: i32) -> Option<char> {
        let c = self.current_offset as i32;

        // Trying to look after the stream
        if c + idx > self.length as i32 {
            return None;
        }

        // Trying to look before the stream
        if c + idx < 0 {
            return None;
        }

        Some(self.buffer[(c + idx) as usize])
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
        assert_eq!(is.read_char().unwrap(), 'f');
        assert_eq!(is.chars_left(), 2);
        assert_eq!(is.eof(), false);
        assert_eq!(is.read_char().unwrap(), '👽');
        assert_eq!(is.eof(), false);
        assert_eq!(is.chars_left(), 1);
        assert_eq!(is.read_char().unwrap(), 'f');
        assert_eq!(is.eof(), true);
        assert_eq!(is.chars_left(), 0);

        is.reset();
        is.set_encoding(Encoding::ASCII);
        assert_eq!(is.length, 6);
        assert_eq!(is.read_char().unwrap(), 'f');
        assert_eq!(is.read_char().unwrap(), '?');
        assert_eq!(is.read_char().unwrap(), '?');
        assert_eq!(is.read_char().unwrap(), '?');
        assert_eq!(is.read_char().unwrap(), '?');
        assert_eq!(is.read_char().unwrap(), 'f');
        assert_eq!(is.read_char(), None);

        is.unread();
        assert_eq!(is.chars_left(), 1);
        is.unread();
        assert_eq!(is.chars_left(), 2);

        is.reset();
        assert_eq!(is.chars_left(), 6);
        is.unread();
        assert_eq!(is.chars_left(), 6);
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
        is.read_from_str("abc\ndefg\n\nhi\njk\nlmno\n\n\npqrst\nu\nv\nw\n\nxy\nz", Some(Encoding::UTF8));
        assert_eq!(is.length, 40);

        is.seek(7);
        assert_eq!(is.current_offset, 7);
        assert_eq!(is.current_line, 2);
        assert_eq!(is.current_line_offset, 4);

        let c = is.read_char().unwrap();
        assert_eq!('g', c);
        assert_eq!(is.current_offset, 8);
        assert_eq!(is.current_line, 2);
        assert_eq!(is.current_line_offset, 5);

        is.read_char();
        assert_eq!(is.current_offset, 9);
        assert_eq!(is.current_line, 3);
        assert_eq!(is.current_line_offset, 1);

        is.read_char();
        assert_eq!(is.current_offset, 10);
        assert_eq!(is.current_line, 4);
        assert_eq!(is.current_line_offset, 1);

        is.reset();
        assert_eq!(is.current_offset, 0);
        assert_eq!(is.current_line, 1);
        assert_eq!(is.current_line_offset, 1);

        is.seek(100);
        assert_eq!(is.current_offset, 39);
        assert_eq!(is.current_line, 15);
        assert_eq!(is.current_line_offset, 1);
    }

    #[test]
    fn test_seek() {
        let mut is = InputStream::new();
        is.read_from_str("ab👽cd", Some(Encoding::UTF8));
        assert_eq!(is.length, 5);
        assert_eq!(is.chars_left(), 5);
        assert_eq!(is.read_char().unwrap(), 'a');
        assert_eq!(is.read_char().unwrap(), 'b');
        assert_eq!(is.chars_left(), 3);
        is.seek(0);
        assert_eq!(is.chars_left(), 5);
        assert_eq!(is.read_char().unwrap(), 'a');
        assert_eq!(is.read_char().unwrap(), 'b');
        assert_eq!(is.chars_left(), 3);
        is.seek(3);
        assert_eq!(is.chars_left(), 2);
        assert_eq!(is.read_char().unwrap(), 'c');
        assert_eq!(is.read_char().unwrap(), 'd');
        assert_eq!(is.chars_left(), 0);
        assert_eq!(is.eof(), true);

        is.reset();
        assert_eq!(is.look_ahead(0).unwrap(), 'a');
        assert_eq!(is.look_ahead(3).unwrap(), 'c');
        assert_eq!(is.look_ahead(1).unwrap(), 'b');
        assert_eq!(is.look_ahead(100), None);
        assert_eq!(is.look_ahead(-1), None);
        is.seek(4);
        assert_eq!(is.look_ahead(-1).unwrap(), 'c');
    }
}
