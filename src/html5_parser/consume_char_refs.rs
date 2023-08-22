use crate::html5_parser::parse_errors::ParserError;
use crate::html5_parser::token_named_characters::TOKEN_NAMED_CHARS;
use crate::html5_parser::token_replacements::TOKEN_REPLACEMENTS;
use crate::html5_parser::tokenizer::Tokenizer;

use super::tokenizer::CHAR_REPLACEMENT;

// Different states for the character references
pub enum CcrState {
    CharacterReferenceState,
    NamedCharacterReferenceState,
    AmbiguousAmpersandState,
    NumericCharacterReferenceState,
    HexadecimalCharacterReferenceStartState,
    DecimalCharacterReferenceStartState,
    HexadecimalCharacterReferenceState,
    DecimalCharacterReferenceState,
    NumericalCharacterReferenceEndState,
}

impl<'a> Tokenizer<'a> {
    // Consumes a character reference and places this in the tokenizer consume buffer
    // ref: 8.2.4.69 Tokenizing character references
    pub fn consume_character_reference(&mut self, _additional_allowed_char: Option<char>, as_attribute: bool)
    {
        let mut tmp_buf = String::new();
        let mut ccr_state = CcrState::CharacterReferenceState;
        let mut char_ref_code: u32 = 0;

        loop {
            match ccr_state {
                CcrState::CharacterReferenceState => {
                    tmp_buf.clear();

                    let c = self.stream.read_char();
                    match c {
                        None => return,
                        Some('A'..='Z') | Some('a'..='z') | Some('0'..='9') => {
                            self.stream.unread();
                            ccr_state = CcrState::NamedCharacterReferenceState;
                        },
                        Some('#') => {
                            tmp_buf.push(c.unwrap());
                            ccr_state = CcrState::NumericCharacterReferenceState;
                        },
                        _ => {
                            for c in tmp_buf.chars() {
                                self.consume(c);
                            }

                            self.stream.unread();
                            return;
                        }
                    }
                },
                CcrState::NamedCharacterReferenceState => {
                    loop {
                        let c = self.stream.read_char();

                        if c.is_none() {
                            while tmp_buf.len() > 0 {
                                tmp_buf.pop();
                                self.stream.unread();
                            }
                            self.consume('&');
                            return;
                        }

                        tmp_buf.push(c.unwrap());
                        let candidates = self.find_entity_candidates(tmp_buf.as_str());

                        // If we found a complete match and it's the only one we can match, we are done
                        if candidates.len() == 1 && candidates.get(0).unwrap().to_string().len() == tmp_buf.len() {
                            self.consume_string(*TOKEN_NAMED_CHARS.get(candidates.get(0).unwrap()).unwrap());
                            return
                        }

                        // If we find a ; or no more candidates could be found, do a backtrack
                        if c.unwrap() == ';' || candidates.len() == 0 {
                            // found a ;. If we don't match, backtrack to find a match
                            while tmp_buf.len() > 0 {
                                tmp_buf.pop();
                                self.stream.unread();

                                // Found a complete match, this is the longest, so use that one
                                if TOKEN_NAMED_CHARS.contains_key(tmp_buf.as_str()) {
                                    self.consume_string(*TOKEN_NAMED_CHARS.get(tmp_buf.as_str()).unwrap());
                                    return
                                }
                            }

                            // no backtrack matches found
                            self.consume('&');
                            return
                        }
                    }
                }
                CcrState::AmbiguousAmpersandState => {
                    let c = self.stream.read_char();
                    match c {
                        None => return,
                        Some('A'..='Z') | Some('a'..='z') | Some('0'..='9') => {
                            if as_attribute {
                                self.current_attr_value.push(c.unwrap());
                            } else {
                                self.consume(c.unwrap());
                            }
                        },
                        Some(';') => {
                            self.parse_error(ParserError::UnknownNamedCharacterReference);
                            self.stream.unread();
                            return;
                        }
                        _ => {
                            self.stream.unread();
                            return;
                        }
                    }
                }
                CcrState::NumericCharacterReferenceState => {
                    char_ref_code = 0;

                    let c = self.stream.read_char();
                    match c {
                        None => return,
                        Some('X') | Some('x') => {
                            tmp_buf.push(c.unwrap());
                            ccr_state = CcrState::HexadecimalCharacterReferenceStartState;
                        }
                        _ => {
                            self.stream.unread();
                            ccr_state = CcrState::DecimalCharacterReferenceStartState;
                        }
                    }
                }
                CcrState::HexadecimalCharacterReferenceStartState => {
                    let c = self.stream.read_char();
                    match c {
                        None => return,
                        Some('0'..='9') | Some('A'..='Z') | Some('a'..='z') => {
                            self.stream.unread();
                            ccr_state = CcrState::HexadecimalCharacterReferenceState
                        }
                        _ => {
                            self.parse_error(ParserError::AbsenceOfDigitsInNumericCharacterReference);
                            for c in tmp_buf.chars() {
                                self.consume(c);
                            }

                            self.stream.unread();
                            return;
                        }
                    }
                }
                CcrState::DecimalCharacterReferenceStartState => {
                    let c = self.stream.read_char();
                    match c {
                        None => return,
                        Some('0'..='9') => {
                            self.stream.unread();
                            ccr_state = CcrState::DecimalCharacterReferenceState;
                        }
                        _ => {
                            self.parse_error(ParserError::AbsenceOfDigitsInNumericCharacterReference);
                            for c in tmp_buf.chars() {
                                self.consume(c);
                            }

                            self.stream.unread();
                            return;
                        }
                    }
                }
                CcrState::HexadecimalCharacterReferenceState => {
                    let c = self.stream.read_char();
                    match c {
                        None => return,
                        Some('0'..='9') => {
                            char_ref_code *= 16;
                            char_ref_code += c.unwrap() as u32 - 0x30;
                        }
                        Some('A'..='F') => {
                            char_ref_code *= 16;
                            char_ref_code += c.unwrap() as u32 - 0x37;
                        }
                        Some('a'..='f') => {
                            char_ref_code *= 16;
                            char_ref_code += c.unwrap() as u32 - 0x57;
                        }
                        Some(';') => {
                            ccr_state = CcrState::NumericalCharacterReferenceEndState;
                        }
                        _ => {
                            self.parse_error(ParserError::MissingSemicolonAfterCharacterReference);

                            self.stream.unread();
                            ccr_state = CcrState::NumericalCharacterReferenceEndState;
                        }
                    }
                }
                CcrState::DecimalCharacterReferenceState => {
                    let c = self.stream.read_char();
                    match c {
                        None => return,
                        Some('0'..='9') => {
                            char_ref_code *= 10;
                            char_ref_code += c.unwrap() as u32 - 0x30;
                        }
                        Some(';') => {
                            ccr_state = CcrState::NumericalCharacterReferenceEndState;
                        }
                        _ => {
                            self.parse_error(ParserError::MissingSemicolonAfterCharacterReference);

                            self.stream.unread();
                            ccr_state = CcrState::NumericalCharacterReferenceEndState;
                        }
                    }
                }
                CcrState::NumericalCharacterReferenceEndState => {
                    if char_ref_code == 0 {
                        self.parse_error(ParserError::NullCharacterReference);
                        char_ref_code = CHAR_REPLACEMENT as u32;
                    }

                    if char_ref_code > 0x10FFFF {
                        self.parse_error(ParserError::CharacterReferenceOutsideUnicodeRange);
                        char_ref_code = CHAR_REPLACEMENT as u32;
                    }

                    if self.is_surrogate(char_ref_code) {
                        self.parse_error(ParserError::SurrogateCharacterReference);
                        char_ref_code = CHAR_REPLACEMENT as u32;
                    }
                    if self.is_noncharacter(char_ref_code) {
                        self.parse_error(ParserError::NoncharacterCharacterReference);
                        char_ref_code = CHAR_REPLACEMENT as u32;
                    }
                    if self.is_control_char(char_ref_code) {
                        self.parse_error(ParserError::ControlCharacterReference);
                    }

                    if TOKEN_REPLACEMENTS.contains_key(&char_ref_code) {
                        char_ref_code = *TOKEN_REPLACEMENTS.get(&char_ref_code).unwrap() as u32;
                    }

                    tmp_buf = String::new();
                    tmp_buf.push(char::from_u32(char_ref_code).unwrap_or(CHAR_REPLACEMENT));

                    for c in tmp_buf.chars() {
                        self.consume(c);
                    }

                    return;
                }
            }
        }
    }

    fn is_surrogate(&self, num: u32) -> bool
    {
        num >= 0xD800 && num <= 0xDFFF
    }

    fn is_noncharacter(&self, num: u32) -> bool
    {
        (0xFDD0..=0xFDEF).contains(&num) || [
            0xFFFE, 0xFFFF, 0x1FFFE, 0x1FFFF, 0x2FFFE, 0x2FFFF, 0x3FFFE, 0x3FFFF,
            0x4FFFE, 0x4FFFF, 0x5FFFE, 0x5FFFF, 0x6FFFE, 0x6FFFF, 0x7FFFE, 0x7FFFF, 0x8FFFE,
            0x8FFFF, 0x9FFFE, 0x9FFFF, 0xAFFFE, 0xAFFFF, 0xBFFFE, 0xBFFFF, 0xCFFFE, 0xCFFFF,
            0xDFFFE, 0xDFFFF, 0xEFFFE, 0xEFFFF, 0xFFFFE, 0xFFFFF, 0x10FFFE, 0x10FFFF,
        ].contains(&num)
    }

    fn is_control_char(&self, num: u32) -> bool
    {
        // White spaces are ok
        if [0x0009, 0x000A, 0x000C, 0x000D, 0x0020].contains(&num) {
            return false;
        }

        return (0x0000..=0x001F).contains(&num) || (0x007F..=0x009F).contains(&num);
    }

    /*
     * Find candidates in the token named entities that start with the given prefix.
     * For instance: the prefix 'no' would return 'not', 'notin' etc..
     *
     * This will iterate over EACH entity, which takes time. At a later stage, we want to
     * do this with an tree index to make it (very) fast.
     */
    fn find_entity_candidates(&self, prefix: &str) -> Vec<&'static str> {
        let mut found = vec![];

        for key in TOKEN_NAMED_CHARS.keys() {
            if key.starts_with(prefix) {
                found.push(*key);
            }
        }

        return found;
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
        entity_6: ("&#x10;", "\u{10}")                // reserved codepoint
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
        entity_111: ("&copya", "©a")
        entity_112: ("&copya;", "©a;")
        entity_113: ("&#169;", "©")
        // entity_114: ("&copy&", "©&")
        entity_115: ("&copya ", "©a ")
        entity_116: ("&#169X ", "©X ")


        // ChatGPT generated tests
        // entity_200: ("&copy;", "©")
        // entity_201: ("&copy ", "© ")
        // entity_202: ("&#169;", "©")
        // entity_203: ("&#xA9;", "©")
        // entity_204: ("&lt;", "<")
        // entity_205: ("&unknown;", "&unknown;")
        // entity_206: ("&#60;", "<")
        // entity_207: ("&#x3C;", "<")
        // entity_208: ("&amp;", "&")
        // entity_209: ("&euro;", "€")
        // entity_210: ("&gt;", ">")
        // entity_211: ("&reg;", "®")
        // entity_212: ("&#174;", "®")
        // entity_213: ("&#xAE;", "®")
        // entity_214: ("&quot;", "\"")
        // entity_215: ("&#34;", "\"")
        // entity_216: ("&#x22;", "\"")
        // entity_217: ("&apos;", "'")
        // entity_218: ("&#39;", "'")
        // entity_219: ("&#x27;", "'")
        // entity_220: ("&excl;", "!")
        // entity_221: ("&#33;", "!")
        // entity_222: ("&num;", "#")
        // entity_223: ("&#35;", "#")
        // entity_224: ("&dollar;", "$")
        // entity_225: ("&#36;", "$")
        // entity_226: ("&percnt;", "%")
        // entity_227: ("&#37;", "%")
        // entity_228: ("&ast;", "*")
        // entity_229: ("&#42;", "*")
        // entity_230: ("&plus;", "+")
        // entity_231: ("&#43;", "+")
        // entity_232: ("&comma;", ",")
        // entity_233: ("&#44;", ",")
        // entity_234: ("&minus;", "−")
        // entity_235: ("&#45;", "-")
        // entity_236: ("&period;", ".")
        // entity_237: ("&#46;", ".")
        // entity_238: ("&sol;", "/")
        // entity_239: ("&#47;", "/")
        // entity_240: ("&colon;", ":")
        // entity_241: ("&#58;", ":")
        // entity_242: ("&semi;", ";")
        // entity_243: ("&#59;", ";")
        // entity_244: ("&equals;", "=")
        // entity_245: ("&#61;", "=")
        // entity_246: ("&quest;", "?")
        // entity_247: ("&#63;", "?")
        // entity_248: ("&commat;", "@")
        // entity_249: ("&#64;", "@")
        // entity_250: ("&COPY;", "©")
        // entity_251: ("&#128;", "€")
        // entity_252: ("&#x9F;", "Ÿ")
        // entity_253: ("&#31;", "")
        // entity_254: ("&#0;", "�")
        // entity_255: ("&#xD800;", "�")
        // entity_256: ("&unknownchar;", "&unknownchar;")
        // entity_257: ("&#9999999;", "�")
        // entity_259: ("&#11;", "")
    }
}