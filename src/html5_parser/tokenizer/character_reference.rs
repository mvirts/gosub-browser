use crate::html5_parser::input_stream::Element;
use crate::html5_parser::error_logger::ParserError;
use crate::read_char;

extern crate lazy_static;
use lazy_static::lazy_static;
use crate::html5_parser::input_stream::SeekMode::SeekCur;
use crate::html5_parser::tokenizer::{CHAR_REPLACEMENT, Tokenizer};
use crate::html5_parser::tokenizer::replacement_tables::{TOKEN_NAMED_CHARS, TOKEN_REPLACEMENTS};

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

macro_rules! consume_temp_buffer {
    ($self:expr, $as_attribute:expr) => {
        for c in $self.temporary_buffer.clone() {
            if $as_attribute {
                $self.current_attr_value.push(c);
            } else {
                $self.consume(c);
            }
        }
        $self.temporary_buffer.clear();
    };
}

impl<'a> Tokenizer<'a> {
    // Consumes a character reference and places this in the tokenizer consume buffer
    // ref: 8.2.4.69 Tokenizing character references

    // @TODO: fix additional allowed char
    pub fn consume_character_reference(&mut self, _additional_allowed_char: Option<Element>, as_attribute: bool)
    {
        let mut ccr_state = CcrState::CharacterReferenceState;
        let mut char_ref_code: Option<u32> = Some(0);

        loop {
            match ccr_state {
                CcrState::CharacterReferenceState => {
                    self.temporary_buffer = vec!['&'];

                    let c = read_char!(self);
                    match c {
                        // Element::Eof => {
                        //     consume_temp_buffer!(self, as_attribute);
                        //     return
                        // },
                        Element::Utf8('A'..='Z') | Element::Utf8('a'..='z') | Element::Utf8('0'..='9') => {
                            self.stream.unread();
                            ccr_state = CcrState::NamedCharacterReferenceState;
                        },
                        Element::Utf8('#') => {
                            self.temporary_buffer.push(c.utf8());
                            ccr_state = CcrState::NumericCharacterReferenceState;
                        },
                        _ => {
                            consume_temp_buffer!(self, as_attribute);

                            self.stream.unread();
                            return;
                        }
                    }
                },
                CcrState::NamedCharacterReferenceState => {
                    if let Some(entity) = self.find_entity() {

                        self.stream.seek(SeekCur, entity.len() as isize);
                        let c = self.stream.look_ahead(0);
                        if
                            as_attribute &&
                            entity.chars().last().unwrap() != ';' &&
                            c.is_utf8() &&
                            (c.utf8() == '=' || c.utf8().is_ascii_alphanumeric())
                        {
                            // for historical reasons, the codepoints should be flushed as is
                            for c in entity.chars() {
                                self.temporary_buffer.push(c);
                            }

                            consume_temp_buffer!(self, as_attribute);
                            return;
                        }

                        let entity_chars = *TOKEN_NAMED_CHARS.get(entity.as_str()).unwrap();

                        // Flush codepoints consumed as character reference
                        for c in entity_chars.chars() {
                            if as_attribute {
                                self.current_attr_value.push(c);
                            } else {
                                self.consume(c);
                            }
                        }
                        self.temporary_buffer.clear();

                        if entity.chars().last().unwrap() != ';' {
                            // We need to return the position where we expected the ';'
                            self.stream.read_char();    // We can't use skip, as this might interfere with EOF stuff (fix it)
                            self.parse_error(ParserError::MissingSemicolonAfterCharacterReference);
                            self.stream.unread();
                        }

                        return;
                    }

                    consume_temp_buffer!(self, as_attribute);
                    ccr_state = CcrState::AmbiguousAmpersandState;
                }
                CcrState::AmbiguousAmpersandState => {
                    let c = read_char!(self);
                    match c {
                        // Element::Eof => return,
                        Element::Utf8('A'..='Z') | Element::Utf8('a'..='z') | Element::Utf8('0'..='9') => {
                            if as_attribute {
                                self.current_attr_value.push(c.utf8());
                            } else {
                                self.consume(c.utf8());
                            }
                        },
                        Element::Utf8(';') => {
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
                    char_ref_code = Some(0);

                    let c = read_char!(self);
                    match c {
                        // Element::Eof => ccr_state = CcrState::NumericalCharacterReferenceEndState,
                        Element::Utf8('X') | Element::Utf8('x') => {
                            self.temporary_buffer.push(c.utf8());
                            ccr_state = CcrState::HexadecimalCharacterReferenceStartState;
                        }
                        _ => {
                            self.stream.unread();
                            ccr_state = CcrState::DecimalCharacterReferenceStartState;
                        }
                    }
                }
                CcrState::HexadecimalCharacterReferenceStartState => {
                    let c = read_char!(self);
                    match c {
                        // Element::Eof => ccr_state = CcrState::NumericalCharacterReferenceEndState,
                        Element::Utf8('0'..='9') | Element::Utf8('A'..='F') | Element::Utf8('a'..='f') => {
                            self.stream.unread();
                            ccr_state = CcrState::HexadecimalCharacterReferenceState
                        }
                        _ => {
                            self.parse_error(ParserError::AbsenceOfDigitsInNumericCharacterReference);
                            consume_temp_buffer!(self, as_attribute);

                            self.stream.unread();
                            return;
                        }
                    }
                }
                CcrState::DecimalCharacterReferenceStartState => {
                    let c = read_char!(self);
                    match c {
                        Element::Utf8('0'..='9') => {
                            self.stream.unread();
                            ccr_state = CcrState::DecimalCharacterReferenceState;
                        }
                        _ => {
                            self.parse_error(ParserError::AbsenceOfDigitsInNumericCharacterReference);
                            consume_temp_buffer!(self, as_attribute);

                            self.stream.unread();
                            return;
                        }
                    }
                }
                CcrState::HexadecimalCharacterReferenceState => {
                    let c = read_char!(self);
                    match c {
                        // Element::Eof => ccr_state = CcrState::NumericalCharacterReferenceEndState,
                        Element::Utf8('0'..='9') => {
                            let i = c.utf8() as u32 - 0x30;
                            if let Some(value) = char_ref_code {
                                char_ref_code = value
                                    .checked_mul(16)
                                    .and_then(|mul_result| mul_result.checked_add(i));
                            }
                        }
                        Element::Utf8('A'..='F') => {
                            let i = c.utf8() as u32 - 0x37;
                            if let Some(value) = char_ref_code {
                                char_ref_code = value
                                    .checked_mul(16)
                                    .and_then(|mul_result| mul_result.checked_add(i));
                            }
                        }
                        Element::Utf8('a'..='f') => {
                            let i = c.utf8() as u32 - 0x57;
                            if let Some(value) = char_ref_code {
                                char_ref_code = value
                                    .checked_mul(16)
                                    .and_then(|mul_result| mul_result.checked_add(i));
                            }
                        }
                        Element::Utf8(';') => {
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
                    let c = read_char!(self);
                    match c {
                        // Element::Eof => ccr_state = CcrState::NumericalCharacterReferenceEndState,
                        Element::Utf8('0'..='9') => {
                            let i = c.utf8() as u32 - 0x30;
                            if let Some(value) = char_ref_code {
                                char_ref_code = value
                                    .checked_mul(10)
                                    .and_then(|mul_result| mul_result.checked_add(i));
                            }
                        }
                        Element::Utf8(';') => {
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
                    let overflow = char_ref_code.is_none();
                    let mut char_ref_code = char_ref_code.unwrap_or(0);

                    if char_ref_code == 0 && !overflow {
                        self.stream.read_char();
                        self.parse_error(ParserError::NullCharacterReference);
                        char_ref_code = CHAR_REPLACEMENT as u32;
                    }

                    if char_ref_code > 0x10FFFF || overflow {
                        self.stream.read_char();
                        self.parse_error(ParserError::CharacterReferenceOutsideUnicodeRange);
                        self.stream.unread();
                        char_ref_code = CHAR_REPLACEMENT as u32;
                    }

                    if self.is_surrogate(char_ref_code) {
                        self.stream.read_char();
                        self.parse_error(ParserError::SurrogateCharacterReference);
                        self.stream.unread();
                        char_ref_code = CHAR_REPLACEMENT as u32;
                    }
                    if self.is_noncharacter(char_ref_code) {
                        self.stream.read_char();
                        self.parse_error(ParserError::NoncharacterCharacterReference);
                        self.stream.unread();
                        // char_ref_code = CHAR_REPLACEMENT as u32;
                    }
                    if self.is_control_char(char_ref_code) || char_ref_code == 0x0D {
                        self.stream.read_char();
                        self.stream.read_char();
                        self.parse_error(ParserError::ControlCharacterReference);
                        // self.stream.unread();
                        self.stream.unread();

                        if TOKEN_REPLACEMENTS.contains_key(&char_ref_code) {
                            char_ref_code = *TOKEN_REPLACEMENTS.get(&char_ref_code).unwrap() as u32;
                        }
                    }

                    self.temporary_buffer = vec![char::from_u32(char_ref_code).unwrap_or(CHAR_REPLACEMENT)];
                    consume_temp_buffer!(self, as_attribute);

                    return;
                }
            }
        }
    }

    pub(crate) fn is_surrogate(&self, num: u32) -> bool
    {
        num >= 0xD800 && num <= 0xDFFF
    }

    pub(crate) fn is_noncharacter(&self, num: u32) -> bool
    {
        (0xFDD0..=0xFDEF).contains(&num) || [
            0xFFFE, 0xFFFF, 0x1FFFE, 0x1FFFF, 0x2FFFE, 0x2FFFF, 0x3FFFE, 0x3FFFF,
            0x4FFFE, 0x4FFFF, 0x5FFFE, 0x5FFFF, 0x6FFFE, 0x6FFFF, 0x7FFFE, 0x7FFFF, 0x8FFFE,
            0x8FFFF, 0x9FFFE, 0x9FFFF, 0xAFFFE, 0xAFFFF, 0xBFFFE, 0xBFFFF, 0xCFFFE, 0xCFFFF,
            0xDFFFE, 0xDFFFF, 0xEFFFE, 0xEFFFF, 0xFFFFE, 0xFFFFF, 0x10FFFE, 0x10FFFF,
        ].contains(&num)
    }

    pub(crate) fn is_control_char(&self, num: u32) -> bool
    {
        // White spaces are ok
        if [0x0009, 0x000A, 0x000C, 0x000D, 0x0020].contains(&num) {
            return false;
        }

        return (0x0001..=0x001F).contains(&num) || (0x007F..=0x009F).contains(&num);
    }

    // Finds the longest entity from the current position in the stream. Returns the entity
    // replacement OR None when no entity has been found.
    fn find_entity(&mut self) -> Option<String> {
        let s= self.stream.look_ahead_slice(*LONGEST_ENTITY_LENGTH);
        for i in (0..=s.len()).rev() {
            if TOKEN_NAMED_CHARS.contains_key(&s[0..i]) {
                // Move forward with the number of chars matching
                // self.stream.skip(i);
                return Some(String::from(&s[0..i]));
            }
        }
        None
    }
}

lazy_static! {
    // Returns the longest entity in the TOKEN_NAMED_CHARS map (this could be a const actually)
    static ref LONGEST_ENTITY_LENGTH: usize = {
        TOKEN_NAMED_CHARS.keys().map(|key| key.len()).max().unwrap_or(0)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use std::cell::RefCell;
    use crate::html5_parser::input_stream::InputStream;
    use crate::html5_parser::error_logger::ErrorLogger;

    macro_rules! entity_tests {
        ($($name:ident : $value:expr)*) => {
            $(
                #[test]
                fn $name() {
                    let (input, expected) = $value;

                    let mut is = InputStream::new();
                    is.read_from_str(input, None);

                    let error_logger = Rc::new(RefCell::new(ErrorLogger::new()));
                    let mut tokenizer = Tokenizer::new(&mut is, None, error_logger.clone());

                    let token = tokenizer.next_token();
                    assert_eq!(expected, token.to_string());
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
        entity_6: ("&#x10;", "�")                // reserved codepoint
        entity_7: ("&#;", "&#;")
        entity_8: ("&;", "&;")
        entity_9: ("&", "&")
        entity_10: ("&#x1;", "�")                // reserved codepoint
        entity_11: ("&#x0008;", "�")             // reserved codepoint
        entity_12: ("&#0008;", "�")              // reserved codepoint
        entity_13: ("&#8;", "�")                 // reserved codepoint
        entity_14: ("&#x0009;", "\t")
        entity_15: ("&#x007F;", "�")             // reserved codepoint
        entity_16: ("&#xFDD0;", "�")             // reserved codepoint

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
        entity_109: ("&copy", "©")
        entity_110: ("&copy ", "© ")
        entity_111: ("&copya", "©a")
        entity_112: ("&copya;", "©a;")
        entity_113: ("&#169;", "©")
        // entity_114: ("&copy&", "©&")
        entity_115: ("&copya ", "©a ")
        entity_116: ("&#169X ", "©X ")


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