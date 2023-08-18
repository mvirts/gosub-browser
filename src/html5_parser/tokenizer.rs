use crate::html5_parser::input_stream::InputStream;
use crate::html5_parser::token::Token;
use crate::html5_parser::token_states::State;

// Constants that are not directly captured as visible chars
pub const CHAR_TAB: char = '\u{0009}';
pub const CHAR_LF: char = '\u{000A}';
pub const CHAR_FF: char = '\u{000C}';
pub const CHAR_SPACE: char = '\u{0020}';
pub const CHAR_REPLACEMENT: char = '\u{FFFD}';

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
            // println!("state: {:?}", self.state);
            // println!("consumed: {:?}", self.consumed);

            // Something is already in the token buffer, so we can return it.
            if self.token_queue.len() > 0 {
                return
            }

            match self.state {
                State::DataState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::TextToken { value: self.get_consumed_str().clone() });
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        '&' => {
                            self.state = State::CharacterReferenceInDataState;

                            // if self.is_consumed() {
                            //     self.token_queue.push(Token::TextToken { value: self.get_consumed_str() });
                            //     self.clear_consume_buffer();
                            //     // return;
                            // }
                        },
                        '<' => {
                            self.state = State::TagOpenState;

                            if self.is_consumed() {
                                self.token_queue.push(Token::TextToken { value: self.get_consumed_str().clone() });
                                self.clear_consume_buffer();
                                // return;
                            }
                        },
                        '\u{0000}' => {
                            self.parse_error("NUL value encountered in data");
                        }
                        _ => self.consume(c),
                    }
                }
                State::CharacterReferenceInDataState => {
                    // consume character reference
                    _ = self.consume_character_reference(None, false);
                    self.state = State::DataState;
                }
                State::RcDataState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        '&' => self.state = State::CharacterReferenceInRcDataState,
                        '<' => self.state = State::RcDataLessThanSignState,
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in RC data");
                        }
                        _ => self.consume(c),
                    }
                }
                State::CharacterReferenceInRcDataState => {
                    // consume character reference
                    _ = self.consume_character_reference(None, false);
                    self.state = State::RcDataState;
                }
                State::RawTextState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        '<' => self.state = State::RawTextLessThanSignState,
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in raw text");
                            self.consume(CHAR_REPLACEMENT);
                            // return;
                        }
                        _ => self.consume(c),
                    }
                }
                State::ScriptDataState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        '<' => self.state = State::ScriptDataLessThenSignState,
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in script data");
                            self.consume(CHAR_REPLACEMENT);
                            // return;
                        }
                        _ => self.consume(c),
                    }
                }
                State::PlaintextState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in plain text stream");
                            self.consume(CHAR_REPLACEMENT);
                            // return;
                        }
                        _ => self.consume(c),
                    }
                }
                State::TagOpenState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        '!' => self.state = State::MarkupDeclarationOpenState,
                        '/' => self.state = State::EndTagOpenState,
                        'A'..='Z' => {
                            self.current_token = Some(Token::StartTagToken{
                                name: "".into(),
                                is_self_closing: false,
                                attributes: vec![],
                            });

                            self.consume(((c as u8) + 0x20) as char);
                            self.state = State::TagNameState;
                        },
                        'a'..='z' => {
                            self.current_token = Some(Token::StartTagToken{
                                name: "".into(),
                                is_self_closing: false,
                                attributes: vec![],
                            });

                            self.consume(c);
                            self.state = State::TagNameState;
                        }
                        '?' => {
                            self.parse_error("questionmark encountered during tag opening");
                            self.state = State::BogusCommentState;
                        }
                        _ => {
                            self.parse_error("unexpected token encountered during tag opening");
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
                            self.parse_error("End of stream reached");
                            self.consume('<');
                            self.consume('/');

                            self.state = State::DataState;
                            continue;
                        }
                    };

                    match c {
                        'A'..='Z' => {
                            // consume lower case
                            self.consume(((c as u8) + 0x20) as char);
                            self.state = State::TagNameState;
                        },
                        'a'..='z' => {
                            self.consume(c);
                            self.state = State::TagNameState;
                        }
                        '>' => {
                            self.parse_error("unexpected > encountered during tag opening");
                            self.state = State::DataState;
                        }
                        _ => {
                            self.parse_error("unexpected character encountered during tag opening");
                            self.state = State::BogusCommentState;
                        }
                    }
                }
                State::TagNameState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        CHAR_TAB | CHAR_LF | CHAR_FF | CHAR_SPACE => {
                            self.state = State::BeforeAttributeNameState;
                        },
                        '/' => self.state = State::SelfClosingStartState,
                        '>' => {
                            let new_name = self.get_consumed_str().clone();
                            match &mut self.current_token.as_mut().unwrap() {
                                Token::StartTagToken { name, .. } => {
                                    *name = new_name;
                                }
                                _ => {
                                    // @TODO: this was not a starttagtoken
                                }
                            }

                            self.clear_consume_buffer();
                            // We are cloning the current token before we send it to the token_queue. This might be inefficient.
                            self.token_queue.push(self.current_token.clone().unwrap());
                            self.current_token = None;
                            self.state = State::DataState;
                            // return;
                        },
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in tag name");
                            self.consume(CHAR_REPLACEMENT);
                        },
                        'A'..='Z' => {
                            self.consume(((c as u8) + 0x20) as char);
                        }
                        _ => self.consume(c),
                    }
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
                State::BeforeAttributeNameState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        CHAR_TAB | CHAR_LF | CHAR_FF | CHAR_SPACE => {
                            // Ignore
                        },
                        '/' => self.state = State::SelfClosingStartState,
                        '>' => {
                            let new_name = self.get_consumed_str().clone();
                            match &mut self.current_token.as_mut().unwrap() {
                                Token::StartTagToken { name, .. } => {
                                    *name = new_name;
                                }
                                _ => {
                                    // @TODO: this was not a starttagtoken
                                }
                            }

                            self.clear_consume_buffer();
                            // We are cloning the current token before we send it to the token_queue. This might be inefficient.
                            self.token_queue.push(self.current_token.clone().unwrap());
                            self.current_token = None;
                            self.state = State::DataState;

                            // return;
                        },
                        'A'..='Z' => {
                            // consume lower case
                            self.consume(((c as u8) + 0x20) as char);
                            self.state = State::AttributeNameState;
                        },
                        '\u{0000}' => {
                            self.parse_error("NUL encountered while starting attribute name");
                            // @TODO: push attribute name is CHAR_REPLACEMENT and value = null
                        },
                        '"' | '\'' | '<' | '=' => {
                            self.parse_error("unexpected token found when starting attribute name");
                            // Start new attribute in current tag, set` name to 'c'
                            self.consume(c);
                            self.state = State::AttributeNameState;
                        }
                        _ => {
                            // Start new attribute in current tag, set name to 'c'
                            self.consume(c);
                            self.state = State::AttributeNameState;
                        },
                    }
                }
                State::AttributeNameState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        CHAR_TAB | CHAR_LF | CHAR_FF | CHAR_SPACE => self.state = State::AfterAttributeNameState,
                        '/' => self.state = State::SelfClosingStartState,
                        '=' => {
                            self.state = State::BeforeAttributeValueState;
                        },
                        '>' => {
                            self.clear_consume_buffer();
                            // We are cloning the current token before we send it to the token_queue. This might be inefficient.
                            self.token_queue.push(self.current_token.clone().unwrap());
                            self.current_token = None;
                            self.state = State::DataState;

                            // return;
                        }
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in attribute name");
                            self.consume(CHAR_REPLACEMENT);
                        },
                        'A'..='Z' => {
                            self.consume(((c as u8) + 0x20) as char);
                        },
                        '"' | '\'' | '<' => {
                            self.parse_error("unexpected token found when starting attribute name");
                            self.consume(c);
                        }
                        _ => self.consume(c),
                    }
                }
                State::BeforeAttributeValueState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        CHAR_TAB | CHAR_LF | CHAR_FF | CHAR_SPACE => {
                            // Ignore
                        },
                        '"' => self.state = State::AttributeValueDoubleQuotedState,
                        '&' => {
                            self.stream.unread();
                            self.state = State::AttributeValueUnquotedState;
                        },
                        '\'' => {
                            self.state = State::AttributeValueSingleQuotedState;
                        }
                        '>' => {
                            self.parse_error("unexpected > encountered in before attribute value state");

                            let new_name = self.get_consumed_str().clone();
                            match &mut self.current_token.as_mut().unwrap() {
                                Token::StartTagToken { name, .. } => {
                                    *name = new_name;
                                }
                                _ => {
                                    // @TODO: this was not a starttagtoken
                                }
                            }

                            self.clear_consume_buffer();
                            // We are cloning the current token before we send it to the token_queue. This might be inefficient.
                            self.token_queue.push(self.current_token.clone().unwrap());
                            self.current_token = None;
                            self.state = State::DataState;
                        },
                        '<' | '=' | '`' => {
                            self.parse_error("unexpected character encountered in before attribute value state");
                            self.consume(c);
                        }
                        _ => {
                            self.consume(c);
                        },
                    }
                }
                State::AttributeValueDoubleQuotedState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        '"' => self.state = State::AfterAttributeValueQuotedState,
                        '&' => {
                            _ = self.consume_character_reference(Some('"'), true);
                        },
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in attribute value");
                            self.consume(CHAR_REPLACEMENT);
                        },
                        _ => {
                            self.consume(c);
                        },
                    }
                }
                State::AttributeValueSingleQuotedState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        '\'' => self.state = State::AfterAttributeValueQuotedState,
                        '&' => {
                            _ = self.consume_character_reference(Some('\''), true);
                        },
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in attribute value");
                            self.consume(CHAR_REPLACEMENT);
                        },
                        _ => {
                            self.consume(c);
                        },
                    }
                }
                //State::AttributeValueUnquotedState => {}
                // State::CharacterReferenceInAttributeValueState => {}
                State::AfterAttributeValueQuotedState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        CHAR_TAB | CHAR_LF | CHAR_FF | CHAR_SPACE => {
                            self.state = State::BeforeAttributeNameState
                        },
                        '\'' => self.state = State::SelfClosingStartState,
                        '>' => {
                            self.state = State::DataState;
                            // @TODO: Emit CURRENT TAG TOKEN
                        },
                        _ => {
                            self.parse_error("unexpected character encountered in the after attribute value state");
                            self.state = State::BeforeAttributeNameState;
                            self.stream.unread();
                        },
                    }
                }
                State::SelfClosingStartState => {
                    let c = match self.stream.read_char() {
                        Some(c) => c,
                        None => {
                            self.parse_error("End of stream reached");
                            self.token_queue.push(Token::EofToken);
                            continue;
                        }
                    };

                    match c {
                        '>' => {
                            let new_name = self.get_consumed_str().clone();
                            match &mut self.current_token.as_mut().unwrap() {
                                Token::StartTagToken { name, is_self_closing, .. } => {
                                    *name = new_name;
                                    *is_self_closing = true;
                                }
                                _ => {
                                    // @TODO: this was not a starttagtoken
                                }
                            }

                            self.clear_consume_buffer();
                            // We are cloning the current token before we send it to the token_queue. This might be inefficient.
                            self.token_queue.push(self.current_token.clone().unwrap());
                            self.current_token = None;
                            self.state = State::DataState;

                            // return;
                        }
                        '\u{0000}' => {
                            self.parse_error("NUL encountered in attribute name");
                            self.consume(CHAR_REPLACEMENT);
                        },
                        'A'..='Z' => {
                            self.consume(((c as u8) + 0x20) as char);
                        },
                        '"' | '\'' | '<' => {
                            self.parse_error("unexpected token found when starting attribute name");
                            self.consume(c);
                        }
                        _ => self.consume(c),
                    }
                }
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
                _ => {
                    panic!("state {:?} not implemented", self.state);
                }
            }
        }
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
        return self.consumed.iter().collect();
    }

    // Returns true if there is anything in the consume buffer
    pub fn is_consumed(&self) -> bool {
        return self.consumed.len() > 0;
    }

    // Clears the current consume buffer
    pub(crate) fn clear_consume_buffer(&mut self) {
        self.consumed.clear()
    }

    // Creates a parser log error message
    pub(crate) fn parse_error(&mut self, _str: &str) {
        // Add to parse log
        println!("parse_error on offset {}: {}", self.stream.tell(), _str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html5_parser::token::{Token, TokenTrait, TokenType};

    macro_rules! test_start_token {
        ($($name:ident : $value:expr)*) => {
            $(
                #[test]
                fn $name() {
                    let (input, name, is_self_closing, attributes, message) = $value;

                    let mut is = InputStream::new();
                    is.read_from_str(input, None);
                    let mut tkznr = Tokenizer::new(&mut is);
                    let t = tkznr.next_token();
                    assert!(t == Token::StartTagToken{ name: String::from(name), is_self_closing, attributes}, "{}", message);
                }
            )*
        }
    }

    macro_rules! test_text_token {
        ($($name:ident : $value:expr)*) => {
            $(
                #[test]
                fn $name() {
                    let (input, value, message) = $value;

                    let mut is = InputStream::new();
                    is.read_from_str(input, None);
                    let mut tkznr = Tokenizer::new(&mut is);
                    let t = tkznr.next_token();
                    assert!(t == Token::TextToken{ value: String::from(value)}, "{}", message);
                }
            )*
        }
    }

    #[test]
    fn test_tokens() {
        let t = Token::CommentToken {
            value: "this is a comment".into(),
        };
        assert_eq!("<!-- this is a comment -->", t.to_string());

        let t = Token::TextToken {
            value: "this is a string".into(),
        };
        assert_eq!("this is a string", t.to_string());

        let t = Token::StartTagToken {
            name: "tag".into(),
            is_self_closing: true,
            attributes: Vec::new(),
        };
        assert_eq!("<tag />", t.to_string());

        let t = Token::StartTagToken {
            name: "tag".into(),
            is_self_closing: false,
            attributes: Vec::new(),
        };
        assert_eq!("<tag>", t.to_string());

        let t = Token::EndTagToken {
            name: "tag".into(),
        };
        assert_eq!("</tag>", t.to_string());

        let t = Token::DocTypeToken {
            name: "html".into(),
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
        is.read_from_str("<bar >< bar><bar/><a> <b> <foo> <FOO> <bar > <bar/> <  bar >", None);
        let mut tkznr = Tokenizer::new(&mut is);

        for _ in 1..20 {
            let t = tkznr.next_token();
            println!("--> Token type: {:?}", t.type_of());
            match t {
                Token::DocTypeToken { .. } => {}
                Token::StartTagToken { name, is_self_closing, .. } => {
                    println!("name: '{}'  self_closing: {}", name, is_self_closing);
                }
                Token::EndTagToken { .. } => {}
                Token::CommentToken { .. } => {}
                Token::TextToken { value } => {
                    println!("'{}'", value);
                }
                Token::EofToken => {}
            }
        }
    }

    test_start_token! {
        // start_test_1: ("<div>", "div", false, vec![], "Basic tag")
        start_test_2: ("<img src=\"image.jpg\">", "img", false, vec![("src".into(), "image.jpg".into())], "Tag with a quoted attribute")
        // start_test_3: ("<a href='http://example.com'>", "a", false, vec![("src".into(), "image.jpg".into())], "Tag with single-quoted attribute")
        // start_test_4: ("<name attr=value>", "name", false, vec![("attr".into(), "value".into())], "Tag with an unquoted attribute")
        // start_test_5: ("<br/>", "br", true, vec![], "Self-closing tag")
        // start_test_6: ("<article data-id=\"5\">", "article", false, vec![("data-id".into(), "5".into())], "Data attribute")
        // start_test_7: ("<SVG>", "svg", false, vec![], "Uppercase tag name")
        // start_test_8: ("<FooBaR>", "foobar", false, vec![], "Mixed case tag name")
        // start_test_9: ("<span class='highlight'>", "span", false, vec![("class".into(), "highlight".into())], "Single-quoted attribute value")
        // start_test_10: ("<link rel=\"stylesheet\" href=\"styles.css\">", "link", false, vec![("rel".into(), "stylesheet".into()), ("href".into(), "styles.css".into())], "Multiple attributes")
        // start_test_11: ("<audio controls>", "audio", false, vec![("controls".into(), "".into())], "Boolean attribute")
        // start_test_12: ("<area href=\"#\" alt=\"Link\">", "a", false, vec![("href".into(), "#".into()), ("alt".into(), "Links".into())], "Tag with multiple attributes, including a fragment URL")
        // start_test_13: ("<canvas id=\"myCanvas\">", "canvas", false, vec![("id".into(), "myCanvas".into())], "CamelCase attribute")
    }

    test_text_token! {
        text_test_1: ("< space>", "< space>", "Tag with spaces in the name")
        text_test_2: ("<123>", "<123>", "Name starting with numbers")
        text_test_3: ("<tag-name>", "<tag-name>", "Tag with a hyphen in its name")
    }

    /*
    <div> - Basic tag
    <img src="image.jpg"> - Tag with a quoted attribute
    <a href='http://example.com'> - Tag with single-quoted attribute
    < space> - Tag with spaces in the name
    <123> - Name starting with numbers
    <name attr=value> - Tag with an unquoted attribute
    <tag-name> - Tag with a hyphen in its name
    </invalid-start> - Invalid starting with a closing bracket
    <br/> - Self-closing tag
    <article data-id="5"> - Data attribute
    <SVG> - Uppercase tag name
    <input type=text> - Unquoted attribute with no special characters
    <span class='highlight'> - Single-quoted attribute value
    <link rel="stylesheet" href="styles.css"> - Multiple attributes
    <audio controls> - Boolean attribute
    <area href="#" alt="Link"> - Tag with multiple attributes, including a fragment URL
    <bdo dir="rtl"> - Enumeration attribute
    <canvas id="myCanvas"> - CamelCase attribute
    <colgroup span="2"> - Numeric attribute
    <command type="command" label="Button"> - Tag with two different attribute types
    <datalist id="list"> - Tag with an ID
    <details open> - Tag with a boolean attribute
    <font face="Arial" color="red"> - Deprecated tag with multiple attributes



    <div - Missing closing angle bracket.
    < > - Empty tag name with spaces.
    <img src="image.jpg - Missing closing angle bracket with attribute.
    </> - Empty closing tag.
    < space > - Spaces within the tag name.
    <a href=> - Attribute without value.
    <> - Empty tag name.
    <tag-name /> - Space before the self-closing slash.
    <name attr="value - Missing closing double quote for the attribute.
    <name attr='value - Missing closing single quote for the attribute.
    <"invalid"> - Tag name starting with a quote.
    <name attr=value value2> - Two values for one attribute.
    <name attr="value"value2> - No space between attribute-value pairs.
    <name attr="value"attr> - No space between attributes.
    </ name> - Space in closing tag name.
    <name/ > - Invalid space in a self-closing tag.
    <name attr=> - Equals sign without a corresponding attribute value.
    <name attr=="value"> - Double equals signs.
    <name name="value"="value"> - Equals sign before attribute name.
    <name "attr"="value"> - Quoted attribute name.
     */
}
