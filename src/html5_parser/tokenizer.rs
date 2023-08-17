use crate::html5_parser::input_stream::InputStream;
use crate::html5_parser::token::Token;
use crate::html5_parser::token_states::State;

// Constants that are not directly captured as visible chars
pub const CHAR_TAB: char = '\u{0009}';
pub const CHAR_LF: char = '\u{000A}';
pub const CHAR_FF: char = '\u{000C}';
pub const CHAR_SPACE: char = '\u{0020}';
pub const CHAR_REPLACEMENT: char = '\u{FFFD}';

// Errors produced by the tokenizer
#[derive(Debug)]
pub enum Error {
    NullEncountered,
}

// The tokenizer will read the input stream and emit tokens that can be used by the parser.
pub struct Tokenizer<'a> {
    pub stream: &'a mut InputStream,    // HTML character input stream
    pub state: State,                   // Current state of the tokenizer
    pub consumed: Vec<char>,            // Current consumed characters for current token
                                        // pub emitter: &'a mut dyn Emitter,   // Emitter trait that will emit the tokens during parsing
    pub current_token: Option<Token>,   // Token that is currently in the making (if any)
    pub token_queue: Vec<Token>,        // Queue of emitted tokens. Needed because we can generate multiple tokens during iteration
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a mut InputStream /*, emitter: &'a mut dyn Emitter*/) -> Self {
        return Tokenizer {
            stream: input,
            state: State::DataState,
            consumed: vec![],
            current_token: None,
            token_queue: vec![],
        };
    }

    // Retrieves the next token from the input stream or Token::EOF when the end is reached
    pub(crate) fn next_token(&mut self) -> Token {
        self.consume_stream();

        if self.token_queue.len() == 0 {
            return Token::EofToken{};
        }

        return self.token_queue.remove(0);
    }

    fn consume_stream(&mut self) {
        loop {
            println!("state: {:?}", self.state);
            println!("consumed: {:?}", self.consumed);

            match self.state {
                State::DataState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("EOF");
                            self.token_queue.push(Token::TextToken { value: self.get_consumed_str().to_string().clone() });
                            self.token_queue.push(Token::EofToken);
                            return;
                        }
                    };

                    match c {
                        '&' => self.state = State::CharacterReferenceInDataState,
                        '<' => self.state = State::TagOpenState,
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in stream");
                        }
                        _ => self.consume(c),
                    }
                }
                State::CharacterReferenceInDataState => {
                    // consume character reference
                    self.consume_character_reference(None, false);
                    self.state = State::DataState;
                }
                State::RcDataState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("EOF");
                            self.token_queue.push(Token::EofToken);
                            return;
                        }
                    };

                    match c {
                        '&' => self.state = State::CharacterReferenceInRcDataState,
                        '<' => self.state = State::RcDataLessThanSignState,
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in stream");
                        }
                        _ => self.consume(c),
                    }
                }
                State::CharacterReferenceInRcDataState => {
                    // consume character reference
                    self.consume_character_reference(None, false);
                    self.state = State::RcDataState;
                }
                State::RawTextState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("EOF");
                            self.token_queue.push(Token::EofToken);
                            return;
                        }
                    };

                    match c {
                        '<' => self.state = State::RawTextLessThanSignState,
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in stream");
                            self.consume(CHAR_REPLACEMENT);
                            return;
                        }
                        _ => self.consume(c),
                    }
                }
                State::ScriptDataState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("EOF");
                            self.token_queue.push(Token::EofToken);
                            return;
                        }
                    };

                    match c {
                        '<' => self.state = State::ScriptDataLessThenSignState,
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in stream");
                            self.consume(CHAR_REPLACEMENT);
                            return;
                        }
                        _ => self.consume(c),
                    }
                }
                State::PlaintextState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("EOF");
                            self.token_queue.push(Token::EofToken);
                            return;
                        }
                    };

                    match c {
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in stream");
                            self.consume(CHAR_REPLACEMENT);
                            return;
                        }
                        _ => self.consume(c),
                    }
                }
                State::TagOpenState => {
                    self.current_token = Some(Token::StartTagToken{
                        name: String::new(),
                        is_self_closing: false,
                        attributes: vec![],
                    });

                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("EOF");
                            self.token_queue.push(Token::EofToken);
                            return;
                        }
                    };

                    match c {
                        '!' => self.state = State::MarkupDeclarationOpenState,
                        '/' => self.state = State::EndTagOpenState,
                        'A'..='Z' => {

                        },
                        'a'..='z' => {

                        }
                        '?' => {
                            self.parse_error("questionmark encountered. bogus.");
                            self.state = State::BogusCommentState;
                        }
                        _ => {
                            self.parse_error("parse error");
                            self.consume('<');
                            self.stream.unread();
                            self.state = State::DataState;
                        }
                    }
                }
                State::EndTagOpenState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("EOF");
                            self.token_queue.push(Token::EofToken);
                            return;
                        }
                    };

                    match c {
                        'A'..='Z' => {
                            // consume lower case
                            self.consume(c as u8 - 0x20);
                            self.state = State::TagNameState;
                        },
                        'a'..='z' => {
                            self.consume(c);
                            self.state = State::TagNameState;
                        }
                        '>' => {
                            self.parse_error("> encountered");
                            self.state = State::DataState;
                        }
                        _ => {
                            self.state = State::BogusCommentState;
                        }
                    }
                }
                State::TagNameState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("EOF");
                            self.token_queue.push(Token::EofToken);
                            return;
                        }
                    };

                    match c {
                        CHAR_TAB | CHAR_LF | CHAR_FF | CHAR_SPACE => self.state = State::BeforeAttributeNameState,
                        '/' => self.state = State::SelfClosingStartState,
                        '>' => {
                            self.current_token.name = self.get_consumed_str();
                            self.token_queue.push(*self.current_token);
                            self.current_token = None;

                            self.state = State::DataState;
                        },
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in stream");
                            self.consume(CHAR_REPLACEMENT);
                        },
                        'A'..='Z' => {
                            self.consume(c - 0x20);
                        }
                        _ => self.consume(c),
                    };
                }
                _ => {
                    panic!("state {} not implemented", self.state);
                }
                // State::RcDataLessThanSignState => {}
                // State::RcDataEndTagOpenState => {}
                // State::RcDataEndTagNameState => {}
                // State::RawTextLessThanSignState => {}
                // State::RawTextEndTagOpenState => {}
                // State::RawTextEndTagNameState => {}
                // State::ScriptDataLessThenSignState => {}
                // State::ScriptDataEndTagOpenState => {}
                // State::ScriptDataEndTagNameState => {}
                // State::ScriptDataEscapeStartState => {}
                // State::ScriptDataEscapeStartDashState => {}
                // State::ScriptDataEscapedState => {}
                // State::ScriptDataEscapedDashState => {}
                // State::ScriptDataEscapedLessThanSignState => {}
                // State::ScriptDataEscapedEndTagOpenState => {}
                // State::ScriptDataEscapedEndTagNameState => {}
                // State::ScriptDataDoubleEscapeStartState => {}
                // State::ScriptDataDoubleEscapedState => {}
                // State::ScriptDataDoubleEscapedDashState => {}
                // State::ScriptDataDoubleEscapedDashDashState => {}
                // State::ScriptDataDoubleEscapedLessThanSignState => {}
                // State::ScriptDataDoubleEscapeEndState => {}
                // State::BeforeAttributeNameState => {}
                // State::AttributeNameState => {}
                // State::BeforeAttributeValueState => {}
                // State::AttributeValueDoubleQuotedState => {}
                // State::AttributeValueSingleQuotedState => {}
                // State::AttributeValueUnquotedState => {}
                // State::CharacterReferenceInAttributeValueState => {}
                // State::AfterAttributeValueQuotedState => {}
                // State::SelfClosingStartState => {
                //
                // }
                // State::BogusCommentState => {}
                // State::MarkupDeclarationOpenState => {}
                // State::CommentStartState => {}
                // State::CommentStartDashState => {}
                // State::CommentState => {}
                // State::CommentEndDashState => {}
                // State::CommentEndState => {}
                // State::CommentEndBangState => {}
                // State::DocTypeState => {}
                // State::BeforeDocTypeNameState => {}
                // State::DocTypeNameState => {}
                // State::AfterDocTypeNameState => {}
                // State::AfterDocTypePublicKeywordState => {}
                // State::BeforeDocTypePublicIdentifierState => {}
                // State::DocTypePublicIdentifierDoubleQuotedState => {}
                // State::DocTypePublicIdentifierSingleQuotedState => {}
                // State::AfterDoctypePublicIdentifierState => {}
                // State::BetweenDocTypePublicAndSystemIdentifiersState => {}
                // State::AfterDocTypeSystemKeywordState => {}
                // State::BeforeDocTypeSystemIdentifiedState => {}
                // State::DocTypeSystemIdentifierDoubleQuotedState => {}
                // State::DocTypeSystemIdentifierSingleQuotedState => {}
                // State::AfterDocTypeSystemIdentifiedState => {}
                // State::BogusDocTypeState => {}
                // State::CDataSectionState => {}
            }
        }

        // return Token::Error{error: Error::EndOfStream, span: String::from("")}
    }

    // Consumes the given char
    pub(crate) fn consume(&mut self, c: char) {
        // Add c to the current token data
        self.consumed.push(c)
    }

    // Consumes the given string
    pub(crate) fn consume_string(&mut self, s: String) {
        // Add c to the current token data
        for c in s.chars() {
            self.consumed.push(c)
        }
    }

    // Return the consumed string as a String
    pub fn get_consumed_str(&self) -> String {
        self.consumed.iter().collect()
    }

    // Clears the current consume buffer
    pub(crate) fn clear_consume_buffer(&mut self) {
        self.consumed.clear()
    }

    // Creates a parser log error message
    pub(crate) fn parse_error(&mut self, _str: &str) {
        // Add to parse log
        println!("parse_error: {}", _str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html5_parser::token::{Token, TokenTrait, TokenType};

    #[test]
    fn test_tokens() {
        let t = Token::CommentToken {
            value: String::from("this is a comment"),
        };
        assert_eq!("<!-- this is a comment -->", t.to_string());

        let t = Token::TextToken {
            value: String::from("this is a string"),
        };
        assert_eq!("this is a string", t.to_string());

        let t = Token::StartTagToken {
            name: String::from("tag"),
            is_self_closing: true,
            attributes: Vec::new(),
        };
        assert_eq!("<tag />", t.to_string());

        let t = Token::StartTagToken {
            name: String::from("tag"),
            is_self_closing: false,
            attributes: Vec::new(),
        };
        assert_eq!("<tag>", t.to_string());

        let t = Token::EndTagToken {
            name: String::from("tag"),
        };
        assert_eq!("</tag>", t.to_string());

        let t = Token::DocTypeToken {
            name: String::from("html"),
            force_quirks: true,
            pub_identifier: Option::from(String::from("foo")),
            sys_identifier: Option::from(String::from("bar")),
        };
        assert_eq!("<!DOCTYPE html FORCE_QUIRKS! foo bar />", t.to_string());
    }

    #[test]
    fn test_tokenizer() {
        let mut is = InputStream::new();
        is.read_from_str("This code is &copy; 2023 &#x80;", None);

        let mut tkznr = Tokenizer::new(&mut is);

        let t = tkznr.next_token();
        assert_eq!(TokenType::TextToken, t.type_of());

        if let Token::TextToken { value } = t {
            assert_eq!("This code is © 2023 €", value);
        }

        let t = tkznr.next_token();
        assert_eq!(TokenType::EofToken, t.type_of());
    }

    #[test]
    fn test_tags() {
        let mut is = InputStream::new();
        is.read_from_str("<a> <b> <foo> <FOO> <bar > <bar/> <  bar >", None);

        let mut tkznr = Tokenizer::new(&mut is);

        let t = tkznr.next_token();
        let t = tkznr.next_token();
        let t = tkznr.next_token();
        let t = tkznr.next_token();
        let t = tkznr.next_token();
        let t = tkznr.next_token();
        let t = tkznr.next_token();
        let t = tkznr.next_token();
        let t = tkznr.next_token();
        assert_eq!(TokenType::TextToken, t.type_of());

        if let Token::TextToken { value } = t {
            assert_eq!("This code is © 2023 €", value);
        }

        let t = tkznr.next_token();
        assert_eq!(TokenType::EofToken, t.type_of());
    }
}
