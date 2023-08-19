use crate::html5_parser::token_named_characters::TOKEN_NAMED_CHARS;
use crate::html5_parser::token_replacements::TOKEN_REPLACEMENTS;
use crate::html5_parser::tokenizer::Tokenizer;

use super::tokenizer::CHAR_REPLACEMENT;

// All references are to chapters in https://dev.w3.org/html5/spec-LC/tokenization.html

impl<'a> Tokenizer<'a> {
    // Consumes a character reference and places this in the tokenizer consume buffer
    // ref: 8.2.4.69 Tokenizing character references
    pub fn consume_character_reference(
        &mut self,
        additional_allowed_char: Option<char>,
        as_attribute: bool,
    ) -> Result<(), ()> {
        if as_attribute {
            // When we are inside an attribute context, things (will/might) be different. Not sure how yet.
        }

        // Note that at this point we have a unconsumed '&' which may or may not be consumed.

        let c = match self.stream.read_char() {
            Some(c) => c,
            None => {
                // First char we read is eof, this means we only have to return the &
                self.consume('&');
                return Err(());
            }
        };

        // Characters that aren't allowed
        let mut chars = vec![
            crate::html5_parser::tokenizer::CHAR_TAB,
            crate::html5_parser::tokenizer::CHAR_LF,
            crate::html5_parser::tokenizer::CHAR_FF,
            crate::html5_parser::tokenizer::CHAR_SPACE,
            '<',
            '&',
        ];

        // The name is weird: addiitonal_allowed_chars, but it would be a char that is NOT allowed (?)
        if additional_allowed_char.is_some() {
            chars.push(additional_allowed_char.unwrap())
        }

        if chars.contains(&c) {
            self.stream.unread();
            return Err(());
        }

        // Consume a number when we found &#
        if c == '#' {
            // self.consume('&');
            // self.consume(c);
            if self.consume_number().is_err() {
                self.stream.unread();
                return Err(());
            }

            return Ok(());
        }

        // Consume anything else when we found & with another char after (ie: &raquo;)
        self.stream.unread();
        if self.consume_entity(as_attribute).is_err() {
            self.stream.unread();
            return Err(());
        }

        return Ok(());
    }

    // Consume a number like #x1234, #123 etc
    fn consume_number(&mut self) -> Result<(), ()> {
        let mut str_num = String::new();

        // Save current position for easy recovery
        let cp = self.stream.tell();

        // Is the char a 'X' or 'x', then we must try and fetch hex digits, otherwise just 0..9
        let mut is_hex = false;
        let hex = match self.stream.look_ahead(0) {
            Some(hex) => hex,
            None => {
                return Err(());
            }
        };

        if hex == 'x' || hex == 'X' {
            is_hex = true;

            // Consume the 'x' character
            let _c = match self.stream.read_char() {
                Some(c) => c,
                None => {
                    self.stream.seek(cp);
                    return Err(());
                }
            };
        };

        let mut i = 0;
        loop {
            let c = match self.stream.read_char() {
                Some(c) => c,
                None => {
                    self.stream.seek(cp);
                    return Err(());
                }
            };

            if is_hex && c.is_ascii_hexdigit() {
                str_num.push(c);
                // self.consume(c);
            } else if !is_hex && c.is_ascii_digit() {
                str_num.push(c);
                // self.consume(c);
            } else {
                self.stream.unread();
                break;
            }

            i += 1;
        }

        // Fetch next character
        let c = match self.stream.read_char() {
            Some(c) => c,
            None => {
                self.stream.seek(cp);
                return Err(());
            }
        };

        // Next character MUST be ;
        if c != ';' {
            self.parse_error("expected a ';'");
            self.stream.seek(cp);
            return Err(());
        }

        // self.consume(c);

        // If we found ;. we need to check how many digits we have parsed. It needs to be at least 1,
        if i == 0 {
            self.parse_error("didn't expect #;");
            self.stream.seek(cp);
            self.consume('&');
            return Err(());
        }

        // check if we need to replace the character. First convert the number to a uint, and use that
        // to check if it exists in the replacements table.
        let num = match u32::from_str_radix(&*str_num, if is_hex { 16 } else { 10 }) {
            Ok(n) => n,
            Err(_) => 0, // lets pretend that an invalid value is set to 0
        };

        if TOKEN_REPLACEMENTS.contains_key(&num) {
            self.consume(*TOKEN_REPLACEMENTS.get(&num).unwrap());
            return Ok(());
        }

        // Next, check if we are in the 0xD800..0xDFFF or 0x10FFFF range, if so, replace
        if (num > 0xD800 && num < 0xDFFF) || (num > 0x10FFFFF) {
            self.parse_error("within reserved codepoint range, but replaced");
            self.consume(crate::html5_parser::tokenizer::CHAR_REPLACEMENT);
            return Ok(());
        }

        // Check if it's in a reserved range, in that case, we ignore the data
        if self.in_reserved_number_range(num) {
            self.parse_error("within reserved codepoint range, ignored");
            return Ok(());
        }

        self.consume(std::char::from_u32(num).unwrap_or(CHAR_REPLACEMENT));

        return Ok(());
    }

    // Returns if the given codepoint number is in a reserved range (as defined in
    // https://dev.w3.org/html5/spec-LC/tokenization.html#consume-a-character-reference)
    fn in_reserved_number_range(&self, codepoint: u32) -> bool {
        if (0x1..=0x0008).contains(&codepoint)
            || (0x000E..=0x001F).contains(&codepoint)
            || (0x007F..=0x009F).contains(&codepoint)
            || (0xFDD0..=0xFDEF).contains(&codepoint)
            || (0x000E..=0x001F).contains(&codepoint)
            || (0x000E..=0x001F).contains(&codepoint)
            || (0x000E..=0x001F).contains(&codepoint)
            || (0x000E..=0x001F).contains(&codepoint)
            || (0x000E..=0x001F).contains(&codepoint)
            || [
                0x000B, 0xFFFE, 0xFFFF, 0x1FFFE, 0x1FFFF, 0x2FFFE, 0x2FFFF, 0x3FFFE, 0x3FFFF,
                0x4FFFE, 0x4FFFF, 0x5FFFE, 0x5FFFF, 0x6FFFE, 0x6FFFF, 0x7FFFE, 0x7FFFF, 0x8FFFE,
                0x8FFFF, 0x9FFFE, 0x9FFFF, 0xAFFFE, 0xAFFFF, 0xBFFFE, 0xBFFFF, 0xCFFFE, 0xCFFFF,
                0xDFFFE, 0xDFFFF, 0xEFFFE, 0xEFFFF, 0xFFFFE, 0xFFFFF, 0x10FFFE, 0x10FFFF,
            ]
            .contains(&codepoint)
        {
            return true;
        }

        return false;
    }

    // This will consume an entity that does not start with &# (ie: &raquo; &#copy;)
    fn consume_entity(&mut self, as_attribute: bool) -> Result<(), ()> {
        // Processing is based on the golang.org/x/net/html package

        let mut capture = String::new();

        loop {
            let c = self.stream.read_char();
            match c {
                Some(c) => {
                    capture.push(c);

                    // If we captured [azAZ09], just continue the capture
                    if 'a' <= c && c <= 'z' || 'A' <= c && c <= 'Z' || '0' <= c && c <= '9' {
                        continue;
                    }

                    break;
                }
                None => {
                    self.parse_error("unexpected end of stream");
                    self.consume('&');
                    self.consume_string(capture);
                    return Ok(());
                }
            }
        }

        // At this point, we have a consume buffer with the entity name in it. We need to check if it's a known entity

        if capture.len() == 0 {
            // If we found nohting (ie: &;)
            self.parse_error("expected entity name");
            return Err(());

        // } else if as_attribute {
        // @TODO: implement this
        // If we need to consume an entity as an attribute, we need to check if the next character is a valid
        // attribute stuff
        } else if TOKEN_NAMED_CHARS.contains_key(capture.as_str()) {
            // If we found a known entity, we need to replace it

            let entity = TOKEN_NAMED_CHARS.get(capture.as_str()).unwrap();
            self.consume_string((*entity).to_string());
            return Ok(());
        } else if !as_attribute {
            // If we found some text, but it's not an entity. We decrease the text until we find something that matches an entity.
            let mut max_len = capture.len();

            // Largest entity is 6 chars. We don't need to check for more
            if max_len > 6 {
                max_len = 6;
            }

            for j in (1..=max_len).rev() {
                let substr: String = capture.chars().take(j).collect();
                if TOKEN_NAMED_CHARS.contains_key(substr.as_str()) {
                    let entity = TOKEN_NAMED_CHARS.get(substr.as_str()).unwrap();
                    self.consume_string((*entity).to_string());
                    self.consume_string(capture.chars().skip(j).collect());
                    return Ok(());
                }
            }
        }

        self.consume('&');
        self.consume_string(capture.to_string());
        return Ok(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html5_parser::input_stream::InputStream;

    macro_rules! entity_tests {
        ($($name:ident : $value:expr)*) => {
            $(
                #[test]
                fn $name() {
                    let (input, expected) = $value;

                    let mut is = InputStream::new();
                    is.read_from_str(input, None);
                    let mut tok = Tokenizer::new(&mut is, None);
                    let t = tok.next_token();
                    assert_eq!(expected, t.to_string());
                }
            )*
        }
    }

    entity_tests! {
        // Numbers
        entity_0: ("&#10;", "\n")
        entity_1: ("&#0;", "�")
        entity_2: ("&#x0;", "�")
        entity_3: ("&#xdeadbeef;", "�")     // replace with replacement char
        entity_4: ("&#xd888;", "�")         // replace with replacement char
        entity_5: ("&#xbeef;", "뻯")
        entity_6: ("&#x10;", "")                // reserved codepoint
        entity_7: ("&#;", "&#;")
        entity_8: ("&;", "&;")
        entity_9: ("&", "&")
        entity_10: ("&#x1;", "")                // reserved codepoint
        entity_11: ("&#x0008;", "")             // reserved codepoint
        entity_12: ("&#0008;", "")              // reserved codepoint
        entity_13: ("&#8;", "")                 // reserved codepoint
        entity_14: ("&#x0009;", "\t")
        entity_15: ("&#x007F;", "")             // reserved codepoint
        entity_16: ("&#xFDD0;", "")             // reserved codepoint

        // Entities
        entity_100: ("&copy;", "©")
        entity_101: ("&copyThing;", "©Thing;")
        entity_102: ("&raquo;", "»")
        entity_103: ("&laquo;", "«")
        entity_104: ("&not;", "¬")
        entity_105: ("&notit;", "¬it;")
        entity_106: ("&notin;", "∉")
        entity_107: ("&fo", "&fo")
        entity_108: ("&xxx", "&xxx")
        entity_109: ("&copy", "&copy")
        entity_110: ("&copy ", "© ")
        entity_111: ("&copya", "&copya")
        entity_112: ("&copya;", "©a;")
        entity_113: ("&#169;", "©")
        entity_114: ("&copy&", "©&")
        entity_115: ("&copya ", "©a ")
        // entity_116: ("&#169X ", "&")       // What should this be?


        // ChatGPT generated tests
        entity_200: ("&copy;", "©")
        entity_201: ("&copy ", "© ")
        entity_202: ("&#169;", "©")
        entity_203: ("&#xA9;", "©")
        entity_204: ("&lt;", "<")
        entity_205: ("&unknown;", "&unknown;")
        entity_206: ("&#60;", "<")
        entity_207: ("&#x3C;", "<")
        entity_208: ("&amp;", "&")
        entity_209: ("&euro;", "€")
        entity_210: ("&gt;", ">")
        entity_211: ("&reg;", "®")
        entity_212: ("&#174;", "®")
        entity_213: ("&#xAE;", "®")
        entity_214: ("&quot;", "\"")
        entity_215: ("&#34;", "\"")
        entity_216: ("&#x22;", "\"")
        entity_217: ("&apos;", "'")
        entity_218: ("&#39;", "'")
        entity_219: ("&#x27;", "'")
        entity_220: ("&excl;", "!")
        entity_221: ("&#33;", "!")
        entity_222: ("&num;", "#")
        entity_223: ("&#35;", "#")
        entity_224: ("&dollar;", "$")
        entity_225: ("&#36;", "$")
        entity_226: ("&percnt;", "%")
        entity_227: ("&#37;", "%")
        entity_228: ("&ast;", "*")
        entity_229: ("&#42;", "*")
        entity_230: ("&plus;", "+")
        entity_231: ("&#43;", "+")
        entity_232: ("&comma;", ",")
        entity_233: ("&#44;", ",")
        entity_234: ("&minus;", "−")
        entity_235: ("&#45;", "-")
        entity_236: ("&period;", ".")
        entity_237: ("&#46;", ".")
        entity_238: ("&sol;", "/")
        entity_239: ("&#47;", "/")
        entity_240: ("&colon;", ":")
        entity_241: ("&#58;", ":")
        entity_242: ("&semi;", ";")
        entity_243: ("&#59;", ";")
        entity_244: ("&equals;", "=")
        entity_245: ("&#61;", "=")
        entity_246: ("&quest;", "?")
        entity_247: ("&#63;", "?")
        entity_248: ("&commat;", "@")
        entity_249: ("&#64;", "@")
        entity_250: ("&COPY;", "©")
        entity_251: ("&#128;", "€")
        entity_252: ("&#x9F;", "Ÿ")
        entity_253: ("&#31;", "")
        entity_254: ("&#0;", "�")
        entity_255: ("&#xD800;", "�")
        entity_256: ("&unknownchar;", "&unknownchar;")
        entity_257: ("&#9999999;", "�")
        entity_259: ("&#11;", "")
    }
}
