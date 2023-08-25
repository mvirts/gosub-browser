use crate::html5_parser::input_stream::InputStream;
use crate::html5_parser::parse_errors::ParserError;
use crate::html5_parser::token::Token;
use crate::html5_parser::token_states::State;

// Constants that are not directly captured as visible chars
pub const CHAR_NUL: char = '\u{0000}';
pub const CHAR_TAB: char = '\u{0009}';
pub const CHAR_LF: char = '\u{000A}';
pub const CHAR_CR: char = '\u{000D}';
pub const CHAR_FF: char = '\u{000C}';
pub const CHAR_SPACE: char = '\u{0020}';
pub const CHAR_REPLACEMENT: char = '\u{FFFD}';

// The tokenizer will read the input stream and emit tokens that can be used by the parser.
pub struct Tokenizer<'a> {
    pub stream: &'a mut InputStream,    // HTML character input stream
    pub state: State,                   // Current state of the tokenizer
    pub consumed: Vec<char>,            // Current consumed characters for current token
    pub current_attr_name: String,      // Current attribute name that we need to store temporary in case we are parsing attributes
    pub current_attr_value: String,     // Current attribute value that we need to store temporary in case we are parsing attributes
    pub ignore_attribute: bool,         // The currently parsed attribute is to be ignored once completed (because it already exists on the current token)
    pub current_token: Option<Token>,   // Token that is currently in the making (if any)
    pub temporary_buffer: Vec<char>,    // Temporary buffer
    pub token_queue: Vec<Token>,        // Queue of emitted tokens. Needed because we can generate multiple tokens during iteration
    pub errors: Vec<ParseError>,        // Parse errors (if any)
    pub last_start_token: String,       // The last emitted start token (or empty if none)
}

pub struct Options {
    pub initial_state: State,           // Sets the initial state of the tokenizer. Normally only needed when dealing with tests
    pub last_start_tag: String,         // Sets the last starting tag in the tokenizer. Normally only needed when dealing with tests
}

macro_rules! add_to_token_value {
    ($self:expr, $c:expr) => {
        match &mut $self.current_token {
            Some(Token::CommentToken {value, ..}) => {
                value.push($c);
            }
            _ => {},
        }
    }
}

macro_rules! add_to_token_name {
    ($self:expr, $c:expr) => {
        match &mut $self.current_token {
            Some(Token::StartTagToken {name, ..}) => {
                name.push($c);
            }
            Some(Token::EndTagToken {name, ..}) => {
                name.push($c);
            }
            Some(Token::DocTypeToken {name, ..}) => {
                name.push($c);
            }
            _ => {},
        }
    }
}

macro_rules! to_lowercase {
    // Converts A-Z to a-z
    ($c:expr) => {
        ((($c) as u8) + 0x20) as char
    };
}

macro_rules! emit_current_token {
    ($self:expr) => {
        match $self.current_token {
            None => {},
            _ => {
                emit_token!($self, $self.current_token.as_ref().unwrap());
            }
        };
        $self.current_token = None;
    };
}

macro_rules! emit_token {
    ($self:expr, $token:expr) => {
        // Save the start token name if we are pushing it. This helps us in detecting matching tags.
        match $token {
            Token::StartTagToken { name, .. } => {
                $self.last_start_token = String::from(name);
            },
            _ => {}
        }

        // If there is any consumed data, emit this first as a text token
        if $self.has_consumed_data() {
            $self.token_queue.push(Token::TextToken{
                value: $self.get_consumed_str(),
            });
            $self.clear_consume_buffer();
        }

        $self.token_queue.push($token.clone());
    }
}

#[derive(PartialEq)]
pub struct ParseError {
    pub message: String,  // Parse message
    pub line: i64,        // Line number of the error
    pub col: i64,         // Offset on line of the error
    pub offset: i64,      // Position of the error on the line
}

impl<'a> Tokenizer<'a> {
    // Creates a new tokenizer with the given inputstream and additional options if any
    pub fn new(input: &'a mut InputStream /*, emitter: &'a mut dyn Emitter*/, opts: Option<Options>) -> Self {
        return Tokenizer {
            stream: input,
            state: opts.as_ref().map_or(State::DataState, |o| o.initial_state),
            last_start_token: opts.as_ref().map_or(String::new(), |o| o.last_start_tag.clone()),
            consumed: vec![],
            current_token: None,
            token_queue: vec![],
            current_attr_name: String::new(),
            current_attr_value: String::new(),
            temporary_buffer: vec![],
            errors: vec![],
            ignore_attribute: false,
        };
    }

    // Retrieves the next token from the input stream or Token::EOF when the end is reached
    pub fn next_token(&mut self) -> Token {
        self.consume_stream();

        if self.token_queue.len() == 0 {
            return Token::EofToken{};
        }

        return self.token_queue.remove(0);
    }

    // Consumes the input stream. Continues until the stream is completed or a token has been generated.
    fn consume_stream(&mut self) {
        loop {
            // Something is already in the token buffer, so we can return it.
            if self.token_queue.len() > 0 {
                return
            }

            match self.state {
                State::DataState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('&') => self.state = State::CharacterReferenceInDataState,
                        Some('<') => self.state = State::TagOpenState,
                        Some(CHAR_NUL) => {
                            self.consume(c.unwrap());
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                        },
                        None => {
                            // EOF
                            if self.has_consumed_data() {
                                emit_token!(self, Token::TextToken { value: self.get_consumed_str() });
                                self.clear_consume_buffer();
                            }
                            emit_token!(self, Token::EofToken);
                        },
                        _ => self.consume(c.unwrap()),
                    }
                }
                State::CharacterReferenceInDataState => {
                    // @TODO: we get into trouble with &copy&, as the last ampersand will get collected by dataState, and consume_character_reference does not
                    // consume the &.
                    _ = self.consume_character_reference(None, false);
                    self.state = State::DataState;
                }
                State::RcDataState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('&') => {
                            self.state = State::CharacterReferenceInRcDataState
                        },
                        Some('<') => self.state = State::RcDataLessThanSignState,
                        None => {
                            if self.has_consumed_data() {
                                emit_token!(self, Token::TextToken { value: self.get_consumed_str().clone() });
                                self.clear_consume_buffer();
                            }
                            emit_token!(self, Token::EofToken);
                        },
                        Some(CHAR_NUL) => {
                            self.consume(CHAR_REPLACEMENT);
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                        },
                        _ => self.consume(c.unwrap()),
                    }
                }
                State::CharacterReferenceInRcDataState => {
                    // consume character reference
                    _ = self.consume_character_reference(None, false);
                    self.state = State::RcDataState;
                }
                State::RawTextState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('<') => self.state = State::RawTextLessThanSignState,
                        Some(CHAR_NUL) => {
                            self.consume(CHAR_REPLACEMENT);
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                        },
                        None => {
                            // EOF
                            if self.has_consumed_data() {
                                emit_token!(self, Token::TextToken { value: self.get_consumed_str() });
                                self.clear_consume_buffer();
                            }
                            emit_token!(self, Token::EofToken);
                        },
                        _ => self.consume(c.unwrap()),
                    }
                }
                State::ScriptDataState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('<') => self.state = State::ScriptDataLessThenSignState,
                        Some(CHAR_NUL) => {
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                            self.consume(CHAR_REPLACEMENT);
                        },
                        None => {
                            if self.has_consumed_data() {
                                emit_token!(self, Token::TextToken { value: self.get_consumed_str().clone() });
                                self.clear_consume_buffer();
                            }
                            emit_token!(self, Token::EofToken);
                        },
                        _ => self.consume(c.unwrap()),
                    }
                }
                State::PlaintextState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(CHAR_NUL) => {
                            self.consume(CHAR_REPLACEMENT);
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                        },
                        None => {
                            if self.has_consumed_data() {
                                emit_token!(self, Token::TextToken { value: self.get_consumed_str().clone() });
                                self.clear_consume_buffer();
                            }
                            emit_token!(self, Token::EofToken);
                        },
                        _ => self.consume(c.unwrap()),
                    }
                }
                State::TagOpenState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('!') => self.state = State::MarkupDeclarationOpenState,
                        Some('/') => self.state = State::EndTagOpenState,
                        Some(ch @ 'A'..='Z') => {
                            self.current_token = Some(Token::StartTagToken{
                                name: "".into(),
                                is_self_closing: false,
                                attributes: vec![],
                            });

                            add_to_token_name!(self, to_lowercase!(ch));
                            self.state = State::TagNameState;
                        },
                        Some(ch @ 'a'..='z') => {
                            self.current_token = Some(Token::StartTagToken{
                                name: "".into(),
                                is_self_closing: false,
                                attributes: vec![],
                            });

                            add_to_token_name!(self, ch);
                            self.state = State::TagNameState;
                        }
                        Some('?') => {
                            self.current_token = Some(Token::CommentToken{
                                value: "".into(),
                            });
                            self.parse_error(ParserError::UnexpectedQuestionMarkInsteadOfTagName);
                            self.stream.unread();
                            self.state = State::BogusCommentState;
                        }
                        None => {
                            self.parse_error(ParserError::EofBeforeTagName);
                            self.consume('<');
                        },
                        _ => {
                            self.parse_error(ParserError::InvalidFirstCharacterOfTagName);
                            self.consume('<');
                            self.stream.unread();
                            self.state = State::DataState;
                        }
                    }
                }
                State::EndTagOpenState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(ch @ 'A'..='Z') => {
                            self.current_token = Some(Token::EndTagToken{
                                name: "".into(),
                            });

                            add_to_token_name!(self, to_lowercase!(ch));
                            self.state = State::TagNameState;
                        },
                        Some(ch @ 'a'..='z') => {
                            self.current_token = Some(Token::EndTagToken{
                                name: "".into(),
                            });

                            add_to_token_name!(self, ch);
                            self.state = State::TagNameState;
                        },
                        Some('>') => {
                            self.parse_error(ParserError::MissingEndTagName);
                            self.state = State::DataState;
                        },
                        None => {
                            self.parse_error(ParserError::EofBeforeTagName);
                            self.consume('<');
                            self.consume('/');
                            self.state = State::DataState;
                        },
                        _ => {
                            self.parse_error(ParserError::InvalidFirstCharacterOfTagName);
                            self.state = State::BogusCommentState;
                        }
                    }
                }
                State::TagNameState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) => self.state = State::BeforeAttributeNameState,
                        Some('/') => self.state = State::SelfClosingStartState,
                        Some('>') => {
                            emit_current_token!(self);
                            self.state = State::DataState;
                        },
                        Some(ch @ 'A'..='Z') => add_to_token_name!(self, to_lowercase!(ch)),
                        Some(CHAR_NUL) => {
                            add_to_token_name!(self, CHAR_REPLACEMENT);
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                        },
                        None => {
                            self.parse_error(ParserError::EofInTag);
                        },
                        _ => add_to_token_name!(self, c.unwrap()),
                    }
                }
                State::RcDataLessThanSignState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('/') => {
                            self.temporary_buffer = vec![];
                            self.state = State::RcDataEndTagOpenState;
                        },
                        _ => {
                            self.consume('<');
                            self.stream.unread();
                            self.state = State::RcDataState;
                        },
                    }
                }
                State::RcDataEndTagOpenState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(ch @ 'A'..='Z') => {
                            self.current_token = Some(Token::EndTagToken{
                                name: "".into(),
                            });
                            self.temporary_buffer.push(to_lowercase!(ch));
                            self.state = State::RcDataEndTagNameState;
                        },
                        Some(ch @ 'a'..='z') => {
                            self.current_token = Some(Token::EndTagToken{
                                name: "".into(),
                            });
                            self.temporary_buffer.push(ch);
                            self.state = State::RcDataEndTagNameState;
                        }
                        _ => {
                            self.consume('<');
                            self.consume('/');
                            self.stream.unread();
                            self.state = State::RcDataState;
                        },
                    }
                }
                State::RcDataEndTagNameState => {
                    let c = self.stream.read_char();

                    // we use this flag because a lot of matches will actually do the same thing
                    let mut consume_anything_else = false;

                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) => {
                            if self.is_appropriate_end_token(&self.temporary_buffer) {
                                self.state = State::BeforeAttributeNameState;
                            } else {
                                consume_anything_else = true;
                            }
                        },
                        Some('/') => {
                            if self.is_appropriate_end_token(&self.temporary_buffer) {
                                self.state = State::SelfClosingStartState;
                            } else {
                                consume_anything_else = true;
                            }
                        },
                        Some('>') => {
                            if self.is_appropriate_end_token(&self.temporary_buffer) {
                                let s: String = self.temporary_buffer.iter().collect::<String>();
                                self.set_name_in_current_token(s);

                                self.last_start_token = String::new();
                                emit_current_token!(self);
                                self.state = State::DataState;
                            } else {
                                consume_anything_else = true;
                            }
                        },
                        Some(ch @ 'A'..='Z') => {
                            self.temporary_buffer.push(to_lowercase!(ch));
                        }
                        Some(ch @ 'a'..='z') => {
                            self.temporary_buffer.push(ch);
                        }
                        _ => {
                            consume_anything_else = true;
                        },
                    }

                    if consume_anything_else {
                        self.consume('<');
                        self.consume('/');
                        for c in self.temporary_buffer.clone() {
                            self.consume(c);
                        }
                        self.temporary_buffer.clear();

                        self.stream.unread();
                        self.state = State::RcDataState;
                    }
                }
                State::RawTextLessThanSignState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('/') => {
                            self.temporary_buffer = vec![];
                            self.state = State::RawTextEndTagOpenState;
                        },
                        _ => {
                            self.consume('<');
                            self.stream.unread();
                            self.state = State::RawTextState;
                        },
                    }
                }
                State::RawTextEndTagOpenState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(ch @ 'A'..='Z') => {
                            self.current_token = Some(Token::EndTagToken{
                                name: "".into(),
                            });
                            // add_to_token_name!(self, to_lowercase!(ch));
                            self.temporary_buffer.push(to_lowercase!(ch));
                            self.state = State::RawTextEndTagNameState;
                        },
                        Some(ch @ 'a'..='z') => {
                            self.current_token = Some(Token::EndTagToken{
                                name: "".into(),
                            });
                            // add_to_token_name!(self, ch);
                            self.temporary_buffer.push(ch);
                            self.state = State::RawTextEndTagNameState;
                        }
                        _ => {
                            self.consume('<');
                            self.consume('/');
                            self.stream.unread();
                            self.state = State::RawTextState;
                        },
                    }
                }
                State::RawTextEndTagNameState => {
                    let c = self.stream.read_char();

                    // we use this flag because a lot of matches will actually do the same thing
                    let mut consume_anything_else = false;

                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) => {
                            if self.is_appropriate_end_token(&self.temporary_buffer) {
                                self.state = State::BeforeAttributeNameState;
                            } else {
                                consume_anything_else = true;
                            }
                        },
                        Some('/') => {
                            if self.is_appropriate_end_token(&self.temporary_buffer) {
                                self.state = State::SelfClosingStartState;
                            } else {
                                consume_anything_else = true;
                            }
                        },
                        Some('>') => {
                            if self.is_appropriate_end_token(&self.temporary_buffer) {
                                let s: String = self.temporary_buffer.iter().collect::<String>();
                                self.set_name_in_current_token(s);
                                self.last_start_token = String::new();
                                emit_current_token!(self);
                                self.state = State::DataState;
                            } else {
                                consume_anything_else = true;
                            }
                        },
                        Some(ch @ 'A'..='Z') => {
                            // add_to_token_name!(self, to_lowercase!(ch));
                            self.temporary_buffer.push(to_lowercase!(ch));
                        }
                        Some(ch @ 'a'..='z') => {
                            // add_to_token_name!(self, ch);
                            self.temporary_buffer.push(ch);
                        }
                        _ => {
                            consume_anything_else = true;
                        },
                    }

                    if consume_anything_else {
                        self.consume('<');
                        self.consume('/');
                        for c in self.temporary_buffer.clone() {
                            self.consume(c);
                        }
                        self.temporary_buffer.clear();

                        self.stream.unread();
                        self.state = State::RawTextState;
                    }
                }
                State::ScriptDataLessThenSignState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('/') => {
                            self.temporary_buffer = vec![];
                            self.state = State::ScriptDataEndTagOpenState;
                        },
                        Some('!') => {
                            self.consume('<');
                            self.consume('!');
                            self.state = State::ScriptDataEscapeStartState;
                        },
                        _ => {
                            self.consume('<');
                            self.stream.unread();
                            self.state = State::ScriptDataState;
                        },
                    }
                }
                State::ScriptDataEndTagOpenState => {
                    let c = self.stream.read_char();
                    if c.is_none() {
                        self.consume('<');
                        self.consume('/');
                        self.stream.unread();
                        self.state = State::ScriptDataState;
                        continue;
                    }

                    if c.unwrap().is_ascii_alphabetic() {
                        self.current_token = Some(Token::EndTagToken{
                            name: "".into(),
                        });

                        self.stream.unread();
                        self.state = State::ScriptDataEndTagNameState;
                    } else {
                        self.consume('<');
                        self.consume('/');
                        self.stream.unread();
                        self.state = State::ScriptDataState;
                    }
                }
                State::ScriptDataEndTagNameState => {
                    let c = self.stream.read_char();

                    // we use this flag because a lot of matches will actually do the same thing
                    let mut consume_anything_else = false;

                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) => {
                            if self.is_appropriate_end_token(&self.temporary_buffer) {
                                self.state = State::BeforeAttributeNameState;
                            } else {
                                consume_anything_else = true;
                            }
                        },
                        Some('/') => {
                            if self.is_appropriate_end_token(&self.temporary_buffer) {
                                self.state = State::SelfClosingStartState;
                            } else {
                                consume_anything_else = true;
                            }
                        },
                        Some('>') => {
                            if self.is_appropriate_end_token(&self.temporary_buffer) {
                                let s: String = self.temporary_buffer.iter().collect::<String>();
                                self.set_name_in_current_token(s);

                                self.last_start_token = String::new();
                                emit_current_token!(self);
                                self.state = State::DataState;
                            } else {
                                consume_anything_else = true;
                            }
                        },
                        Some(ch @ 'A'..='Z') => {
                            self.temporary_buffer.push(to_lowercase!(ch));
                        }
                        Some(ch @ 'a'..='z') => {
                            self.temporary_buffer.push(ch);
                        }
                        _ => {
                            consume_anything_else = true;
                        },
                    }

                    if consume_anything_else {
                        self.consume('<');
                        self.consume('/');
                        for c in self.temporary_buffer.clone() {
                            self.consume(c);
                        }
                        self.temporary_buffer.clear();

                        self.stream.unread();
                        self.state = State::ScriptDataState;
                    }
                }
                State::ScriptDataEscapeStartState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            self.consume('-');
                            self.state = State::ScriptDataEscapeStartDashState;
                        },
                        _ => {
                            self.stream.unread();
                            self.state = State::ScriptDataState;
                        },
                    }
                }
                State::ScriptDataEscapeStartDashState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            self.consume('-');
                            self.state = State::ScriptDataEscapedDashDashState;
                        },
                        _ => {
                            self.stream.unread();
                            self.state = State::ScriptDataState;
                        },
                    }
                }
                State::ScriptDataEscapedState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            self.consume('-');
                            self.state = State::ScriptDataEscapedDashState;
                        },
                        Some('<') => {
                            self.state = State::ScriptDataEscapedLessThanSignState;
                        },
                        Some(CHAR_NUL) => {
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                            self.consume(CHAR_REPLACEMENT);
                        },
                        None => {
                            self.parse_error(ParserError::EofInScriptHtmlCommentLikeText);
                            self.state = State::DataState;
                        },
                        _ => {
                            self.consume(c.unwrap());
                        },
                    }
                }
                State::ScriptDataEscapedDashState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            self.consume('-');
                            self.state = State::ScriptDataEscapedDashDashState;
                        },
                        Some('<') => {
                            self.state = State::ScriptDataEscapedLessThanSignState;
                        },
                        Some(CHAR_NUL) => {
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                            self.consume(CHAR_REPLACEMENT);
                            self.state = State::ScriptDataEscapedState;
                        },
                        None => {
                            self.parse_error(ParserError::EofInScriptHtmlCommentLikeText);
                            self.state = State::DataState;
                        },
                        _ => {
                            self.stream.unread();
                            self.state = State::ScriptDataEscapedState;
                        },
                    }
                }
                State::ScriptDataEscapedDashDashState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            self.consume('-');
                        },
                        Some('<') => {
                            self.state = State::ScriptDataEscapedLessThanSignState;
                        },
                        Some('>') => {
                            self.consume('>');
                            self.state = State::ScriptDataState;
                        }
                        Some(CHAR_NUL) => {
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                            self.consume(CHAR_REPLACEMENT);
                            self.state = State::ScriptDataEscapedState;
                        },
                        None => {
                            self.parse_error(ParserError::EofInScriptHtmlCommentLikeText);
                            self.state = State::DataState;
                        },
                        _ => {
                            self.stream.unread();
                            self.state = State::ScriptDataEscapedState;
                        },
                    }
                }
                State::ScriptDataEscapedLessThanSignState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('/') => {
                            self.temporary_buffer = vec![];
                            self.state = State::ScriptDataEscapedEndTagOpenState;
                        },
                        _ => {
                            if c.is_some() && c.unwrap().is_ascii_alphabetic() {
                                self.temporary_buffer = vec![];
                                self.consume('<');
                                self.stream.unread();
                                self.state = State::ScriptDataDoubleEscapeStartState;
                                continue;
                            }
                            // anything else
                            self.consume('<');
                            self.stream.unread();
                            self.state = State::ScriptDataEscapedState;
                        },
                    }
                }
                State::ScriptDataEscapedEndTagOpenState => {
                    let c = self.stream.read_char();

                    if c.is_some() && c.unwrap().is_ascii_alphabetic() {
                        self.current_token = Some(Token::EndTagToken{
                            name: "".into(),
                        });

                        self.stream.unread();
                        self.state = State::ScriptDataEscapedEndTagNameState;
                        continue;
                    }

                    // anything else
                    self.consume('<');
                    self.consume('/');
                    self.stream.unread();
                    self.state = State::ScriptDataEscapedState;
                }
                State::ScriptDataEscapedEndTagNameState => {
                    let c = self.stream.read_char();

                    // we use this flag because a lot of matches will actually do the same thing
                    let mut consume_anything_else = false;

                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) => {
                            if self.is_appropriate_end_token(&self.temporary_buffer) {
                                self.state = State::BeforeAttributeNameState;
                            } else {
                                consume_anything_else = true;
                            }
                        },
                        Some('/') => {
                            if self.is_appropriate_end_token(&self.temporary_buffer) {
                                self.state = State::SelfClosingStartState;
                            } else {
                                consume_anything_else = true;
                            }
                        },
                        Some('>') => {
                            if self.is_appropriate_end_token(&self.temporary_buffer) {
                                let s: String = self.temporary_buffer.iter().collect::<String>();
                                self.set_name_in_current_token(s);

                                self.last_start_token = String::new();
                                emit_current_token!(self);
                                self.state = State::DataState;
                            } else {
                                consume_anything_else = true;
                            }
                        },
                        Some(ch @ 'A'..='Z') => {
                            self.temporary_buffer.push(to_lowercase!(ch));
                        }
                        Some(ch @ 'a'..='z') => {
                            self.temporary_buffer.push(ch);
                        }
                        _ => {
                            consume_anything_else = true;
                        },
                    }

                    if consume_anything_else {
                        self.consume('<');
                        self.consume('/');
                        for c in self.temporary_buffer.clone() {
                            self.consume(c);
                        }
                        self.temporary_buffer.clear();

                        self.stream.unread();
                        self.state = State::ScriptDataEscapedState;
                    }
                }
                State::ScriptDataDoubleEscapeStartState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) |
                        Some('/') |
                        Some('>') => {
                            if self.temporary_buffer.iter().collect::<String>().eq("script") {
                                self.state = State::ScriptDataDoubleEscapedState;
                            } else {
                                self.state = State::ScriptDataEscapedState;
                            }
                            self.consume(c.unwrap());
                        }
                        Some(ch @ 'A'..='Z') => {
                            self.temporary_buffer.push(to_lowercase!(ch));
                            self.consume(ch);
                        },
                        Some(ch @ 'a'..='z') => {
                            self.temporary_buffer.push(ch);
                            self.consume(ch);
                        },
                        _ => {
                            self.stream.unread();
                            self.state = State::ScriptDataEscapedState;
                        }
                    }
                },
                State::ScriptDataDoubleEscapedState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            self.consume('-');
                            self.state = State::ScriptDataDoubleEscapedDashState;
                        }
                        Some('<') => {
                            self.consume('<');
                            self.state = State::ScriptDataDoubleEscapedLessThanSignState;
                        },
                        Some(CHAR_NUL) => {
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                            self.consume(CHAR_REPLACEMENT);
                        },
                        None => {
                            self.parse_error(ParserError::EofInScriptHtmlCommentLikeText);
                            self.state = State::DataState;
                        }
                        _ => self.consume(c.unwrap()),
                    }
                }
                State::ScriptDataDoubleEscapedDashState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            self.state = State::ScriptDataDoubleEscapedDashDashState;
                            self.consume('-');
                        }
                        Some('<') => {
                            self.state = State::ScriptDataDoubleEscapedLessThanSignState;
                            self.consume('<');
                        },
                        Some(CHAR_NUL) => {
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                            self.consume(CHAR_REPLACEMENT);
                            self.state = State::ScriptDataDoubleEscapedState;
                        },
                        None => {
                            self.parse_error(ParserError::EofInScriptHtmlCommentLikeText);
                            self.state = State::DataState;
                        }
                        _ => {
                            self.consume(c.unwrap());
                            self.state = State::ScriptDataDoubleEscapedState;
                        },
                    }
                }
                State::ScriptDataDoubleEscapedDashDashState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => self.consume('-'),
                        Some('<') => {
                            self.consume('<');
                            self.state = State::ScriptDataDoubleEscapedLessThanSignState;
                        },
                        Some('>') => {
                            self.consume('>');
                            self.state = State::ScriptDataState;
                        },
                        Some(CHAR_NUL) => {
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                            self.consume(CHAR_REPLACEMENT);
                            self.state = State::ScriptDataDoubleEscapedState;
                        },
                        None => {
                            self.parse_error(ParserError::EofInScriptHtmlCommentLikeText);
                            self.state = State::DataState;
                        },
                        _ => {
                            self.consume(c.unwrap());
                            self.state = State::ScriptDataDoubleEscapedState;
                        },
                    }
                }
                State::ScriptDataDoubleEscapedLessThanSignState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('/') => {
                            self.temporary_buffer = vec![];
                            self.consume('/');
                            self.state = State::ScriptDataDoubleEscapeEndState;
                        }
                        _ => {
                            self.stream.unread();
                            self.state = State::ScriptDataDoubleEscapedState;
                        },
                    }
                }
                State::ScriptDataDoubleEscapeEndState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) |
                        Some('/') |
                        Some('>') => {
                            if self.temporary_buffer.iter().collect::<String>().eq("script") {
                                self.state = State::ScriptDataEscapedState;
                            } else {
                                self.state = State::ScriptDataDoubleEscapedState;
                            }
                            self.consume(c.unwrap());
                        }
                        Some(ch @ 'A'..='Z') => {
                            self.temporary_buffer.push(to_lowercase!(ch));
                            self.consume(ch);
                        },
                        Some(ch @ 'a'..='z') => {
                            self.temporary_buffer.push(ch);
                            self.consume(ch);
                        },
                        _ => {
                            self.stream.unread();
                            self.state = State::ScriptDataDoubleEscapedState;
                        }
                    }
                }
                State::BeforeAttributeNameState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) => {
                            // Ignore character
                        },
                        Some('/') | Some('>') | None => {
                            self.stream.unread();
                            self.state = State::AfterAttributeNameState;
                        },
                        Some('=') => {
                            self.parse_error(ParserError::UnexpectedEqualsSignBeforeAttributeName);
                            self.current_attr_name.clear();
                            self.current_attr_value = String::new();
                            self.stream.unread();
                            self.state = State::AttributeNameState;
                        }
                        _ => {
                            self.current_attr_name.clear();
                            self.current_attr_value = String::new();
                            self.stream.unread();
                            self.state = State::AttributeNameState;
                        },
                    }
                }
                State::AttributeNameState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) |
                        Some('>') |
                        None => {
                            self.stream.unread();
                            self.state = State::AfterAttributeNameState
                        },
                        Some('=') => {
                            self.state = State::BeforeAttributeValueState
                        },
                        Some(ch @ 'A'..='Z') => {
                            self.current_attr_name.push(to_lowercase!(ch));
                        },
                        Some(CHAR_NUL)  => {
                            self.current_attr_name.push(CHAR_REPLACEMENT);
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                        },
                        Some('"') | Some('\'') | Some('<') => {
                            self.parse_error(ParserError::UnexpectedCharacterInAttributeName);
                            self.current_attr_name.push(c.unwrap());
                        },
                        _ => self.current_attr_name.push(c.unwrap()),
                    }
                }
                State::AfterAttributeNameState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) => {
                            // Ignore
                        },
                        Some('/') => self.state = State::SelfClosingStartState,
                        Some('=') => self.state = State::BeforeAttributeValueState,
                        Some('>') => {
                            self.state = State::DataState;
                            emit_current_token!(self);
                        }
                        None => {
                            self.parse_error(ParserError::EofInTag);
                            self.state = State::DataState;
                        },
                        _ => {
                            self.current_attr_name.clear();
                            self.current_attr_value = String::new();
                            self.state = State::AttributeNameState;
                        },
                    }
                },
                State::BeforeAttributeValueState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) => {
                            // Ignore
                        },
                        Some('"') => self.state = State::AttributeValueDoubleQuotedState,
                        Some('&') => {
                            self.state = State::AttributeValueUnquotedState;
                        },
                        Some('\'') => {
                            self.state = State::AttributeValueSingleQuotedState;
                        }
                        Some('>') => {
                            self.parse_error(ParserError::MissingAttributeValue);
                            emit_current_token!(self);
                            self.state = State::DataState;
                        },
                        _ => {
                            self.stream.unread();
                            self.state = State::AttributeValueUnquotedState;
                        },
                    }
                }
                State::AttributeValueDoubleQuotedState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('"') => self.state = State::AfterAttributeValueQuotedState,
                        Some('&') => _ = self.consume_character_reference(Some('"'), true),
                        Some(CHAR_NUL) => {
                            self.current_attr_value.push(CHAR_REPLACEMENT);
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                        },
                        None => {
                            self.parse_error(ParserError::EofInTag);
                            self.state = State::DataState;
                        },
                        _ => {
                            self.current_attr_value.push(c.unwrap());
                        },
                    }
                }
                State::AttributeValueSingleQuotedState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('\'') => self.state = State::AfterAttributeValueQuotedState,
                        Some('&') => _ = self.consume_character_reference(Some('\''), true),
                        Some(CHAR_NUL) => {
                            self.current_attr_value.push(CHAR_REPLACEMENT);
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                        },
                        None => {
                            self.parse_error(ParserError::EofInTag);
                            self.state = State::DataState;
                        },
                        _ => {
                            self.current_attr_value.push(c.unwrap());
                        },
                    }
                }
                State::AttributeValueUnquotedState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) => {
                            self.state = State::BeforeAttributeNameState;
                        },
                        Some('&') => _ = self.consume_character_reference(Some('>'), true),
                        Some('>') => {
                            emit_current_token!(self);
                            self.state = State::DataState;
                        },
                        Some(CHAR_NUL) => {
                            self.current_attr_value.push(CHAR_REPLACEMENT);
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                        },
                        Some('"') | Some('\'') | Some('<') | Some('=') | Some('`') => {
                            self.parse_error(ParserError::UnexpectedCharacterInUnquotedAttributeValue);
                            self.current_attr_value.push(c.unwrap());
                        }
                        None => {
                            self.parse_error(ParserError::EofInTag);
                            self.state = State::DataState;
                        },
                        _ => {
                            self.current_attr_value.push(c.unwrap());
                        },
                    }

                }
                // State::CharacterReferenceInAttributeValueState => {}
                State::AfterAttributeValueQuotedState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) => self.state = State::BeforeAttributeNameState,
                        Some('\'') => self.state = State::SelfClosingStartState,
                        Some('>') => {
                            emit_current_token!(self);
                            self.state = State::DataState;
                        },
                        None => {
                            self.parse_error(ParserError::EofInTag);
                            self.state = State::DataState;
                        },
                        _ => {
                            self.parse_error(ParserError::MissingWhitespaceBetweenAttributes);
                            self.stream.unread();
                            self.state = State::BeforeAttributeNameState;
                        },
                    }
                }
                State::SelfClosingStartState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('>') => {
                            self.set_is_closing_in_current_token(true);
                            emit_current_token!(self);
                            self.state = State::DataState;
                        }
                        None => {
                            self.parse_error(ParserError::EofInTag);
                            self.state = State::DataState;
                        },
                        _ => {
                            self.parse_error(ParserError::UnexpectedSolidusInTag);
                            self.stream.unread();
                            self.state = State::BeforeAttributeNameState;
                        },
                    }
                }
                State::BogusCommentState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('>') => {
                            emit_current_token!(self);
                            self.state = State::DataState;
                        }
                        None => {
                            self.parse_error(ParserError::EofInTag);
                            emit_current_token!(self);
                            self.state = State::DataState;
                        },
                        Some(CHAR_NUL) => {
                            add_to_token_value!(self, CHAR_REPLACEMENT);
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                        }
                        _ => {
                            add_to_token_value!(self, c.unwrap());
                        },
                    }
                }
                State::MarkupDeclarationOpenState => {
                    if self.stream.look_ahead_slice(2) == "--" {
                        self.current_token = Some(Token::CommentToken{
                            value: "".into(),
                        });

                        // Skip the two -- signs
                        self.stream.seek(self.stream.position.offset + 2);

                        self.state = State::CommentStartState;
                        continue;
                    }

                    if self.stream.look_ahead_slice(7) == "DOCTYPE" {
                        self.stream.seek(self.stream.position.offset + 7);
                        self.state = State::DocTypeState;
                        continue;
                    }

                    if self.stream.look_ahead_slice(7) == "[CDATA[" {
                        self.stream.seek(self.stream.position.offset + 7);

                        // @TODO: If there is an adjusted current node and it is not an element in the HTML namespace,
                        // then switch to the CDATA section state. Otherwise, this is a cdata-in-html-content parse error.
                        // Create a comment token whose data is the "[CDATA[" string. Switch to the bogus comment state.
                        self.current_token = Some(Token::CommentToken{
                            value: "[CDATA[".into(),
                        });

                        self.state = State::BogusCommentState;
                        continue;
                    }

                    self.parse_error(ParserError::IncorrectlyOpenedComment);
                    self.current_token = Some(Token::CommentToken{
                        value: "".into(),
                    });

                    self.state = State::BogusCommentState;
                }
                State::CommentStartState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            self.state = State::CommentStartDashState;
                        }
                        Some('>') => {
                            self.parse_error(ParserError::AbruptClosingOfEmptyComment);
                            emit_current_token!(self);
                            self.state = State::DataState;
                        }
                        _ => {
                            self.stream.unread();
                            self.state = State::CommentState;
                        },
                    }
                }
                State::CommentStartDashState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            self.state = State::CommentEndState;
                        }
                        Some('>') => {
                            self.parse_error(ParserError::AbruptClosingOfEmptyComment);
                            emit_current_token!(self);
                            self.state = State::DataState;
                        }
                        None => {
                            self.parse_error(ParserError::EofInTag);
                            emit_current_token!(self);
                            self.state = State::DataState;
                        },
                        _ => {
                            add_to_token_value!(self, '-');
                            self.stream.unread();
                            self.state = State::CommentState;
                        },
                    }
                }
                State::CommentState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('<') => {
                            add_to_token_value!(self, c.unwrap());
                            self.state = State::CommentLessThanSignState;
                        }
                        Some('-') => self.state = State::CommentEndDashState,
                        Some(CHAR_NUL) => {
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                            add_to_token_value!(self, CHAR_REPLACEMENT);
                        }
                        None => {
                            self.parse_error(ParserError::EofInComment);
                            emit_current_token!(self);
                            self.state = State::DataState;
                        }
                        _ => {
                            add_to_token_value!(self, c.unwrap());
                        },
                    }
                }
                State::CommentLessThanSignState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('!') => {
                            add_to_token_value!(self, c.unwrap());
                            self.state = State::CommentLessThanSignBangState;
                        },
                        Some('<') => {
                            add_to_token_value!(self, c.unwrap());
                        },
                        _ => {
                            self.stream.unread();
                            self.state = State::CommentState;
                        },
                    }
                },
                State::CommentLessThanSignBangState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            self.state = State::CommentLessThanSignBangDashState;
                        },
                        _ => {
                            self.stream.unread();
                            self.state = State::CommentState;
                        },
                    }
                },
                State::CommentLessThanSignBangDashState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            self.state = State::CommentLessThanSignBangDashDashState;
                        },
                        _ => {
                            self.stream.unread();
                            self.state = State::CommentEndDashState;
                        },
                    }
                },
                State::CommentLessThanSignBangDashDashState => {
                    let c = self.stream.read_char();
                    match c {
                        None | Some('>') => {
                            self.stream.unread();
                            self.state = State::CommentEndState;
                        },
                        _ => {
                            self.parse_error(ParserError::NestedComment);
                            self.stream.unread();
                            self.state = State::CommentEndState;
                        },
                    }
                },
                State::CommentEndDashState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            self.state = State::CommentEndState;
                        },
                        None => {
                            self.parse_error(ParserError::EofInComment);
                            emit_current_token!(self);
                            self.state = State::DataState;
                        }
                        _ => {
                            add_to_token_value!(self, '-');
                            self.stream.unread();
                            self.state = State::CommentState;
                        },
                    }
                }
                State::CommentEndState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('>') => {
                            emit_current_token!(self);
                            self.state = State::DataState;
                        },
                        Some('!') => self.state = State::CommentEndBangState,
                        Some('-') => add_to_token_value!(self, '-'),
                        None => {
                            self.parse_error(ParserError::EofInComment);
                            emit_current_token!(self);
                            self.state = State::DataState;
                        }
                        _ => {
                            add_to_token_value!(self, '-');
                            add_to_token_value!(self, '-');
                            self.stream.unread();
                            self.state = State::CommentState;
                        }
                    }
                }
                State::CommentEndBangState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('-') => {
                            add_to_token_value!(self, '-');
                            add_to_token_value!(self, '-');
                            add_to_token_value!(self, '!');

                            self.state = State::CommentEndDashState;
                        },
                        Some('>') => {
                            self.parse_error(ParserError::IncorrectlyClosedComment);
                            emit_current_token!(self);
                            self.state = State::DataState;
                        },
                        None => {
                            self.parse_error(ParserError::EofInComment);
                            emit_current_token!(self);
                            self.state = State::DataState;
                        }
                        _ => {
                            add_to_token_value!(self, '-');
                            add_to_token_value!(self, '-');
                            add_to_token_value!(self, '!');
                            self.stream.unread();
                            self.state = State::CommentState;
                        }
                    }
                }
                State::DocTypeState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) => self.state = State::BeforeDocTypeNameState,
                        Some('>') => {
                            self.stream.unread();
                            self.state = State::BeforeDocTypeNameState;
                        },
                        None => {
                            self.parse_error(ParserError::EofInDoctype);

                            emit_token!(self, Token::DocTypeToken{
                                name: "".to_string(),
                                force_quirks: true,
                                pub_identifier: None,
                                sys_identifier: None,
                            });

                            self.state = State::DataState;
                        }
                        _ => {
                            self.parse_error(ParserError::MissingWhitespaceBeforeDoctypeName)
                            self.stream.unread();
                            self.state = State::BeforeDocTypeNameState;
                        }
                    }
                }
                State::BeforeDocTypeNameState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(CHAR_TAB) |
                        Some(CHAR_LF) |
                        Some(CHAR_FF) |
                        Some(CHAR_SPACE) => {
                            // ignore
                        }
                        Some(ch @ 'A'..='Z') => {
                            self.current_token = Some(Token::DocTypeToken{
                                name: "".to_string(),
                                force_quirks: true,
                                pub_identifier: None,
                                sys_identifier: None,
                            });

                            add_to_token_name!(self, to_lowercase!(ch));
                            self.state = State::DocTypeNameState;
                        }
                        Some(CHAR_NUL) => {
                            self.parse_error(ParserError::UnexpectedNullCharacter);
                            self.current_token = Some(Token::DocTypeToken{
                                name: "".to_string(),
                                force_quirks: false,
                                pub_identifier: None,
                                sys_identifier: None,
                            });

                            add_to_token_name!(self, CHAR_REPLACEMENT);
                            self.state = State::DocTypeNameState;
                        },
                        Some('>') => {
                            self.parse_error(ParserError::MissingDoctypeName);
                            emit_token!(Token::DocTypeToken{
                                name: "".to_string(),
                                force_quirks: true,
                                pub_identifier: None,
                                sys_identifier: None,
                            });

                            self.state = State::DataState;
                        },

                        None => {
                            self.parse_error(ParserError::EofInDoctype);

                            emit_token!(self, Token::DocTypeToken{
                                name: "".to_string(),
                                force_quirks: true,
                                pub_identifier: None,
                                sys_identifier: None,
                            });

                            self.state = State::DataState;
                        }
                        _ => {
                            self.current_token = Some(Token::DocTypeToken{
                                name: "".to_string(),
                                force_quirks: false,
                                pub_identifier: None,
                                sys_identifier: None,
                            }));

                            add_to_token_name!(self, c.unwrap());
                            self.state = State::DocTypeNameState;
                        }
                    }
                }
                State::DocTypeNameState => {}
                State::AfterDocTypeNameState => {}
                State::AfterDocTypePublicKeywordState => {}
                State::BeforeDocTypePublicIdentifierState => {}
                State::DocTypePublicIdentifierDoubleQuotedState => {}
                State::DocTypePublicIdentifierSingleQuotedState => {}
                State::AfterDoctypePublicIdentifierState => {}
                State::BetweenDocTypePublicAndSystemIdentifiersState => {}
                State::AfterDocTypeSystemKeywordState => {}
                State::BeforeDocTypeSystemIdentifiedState => {}
                State::DocTypeSystemIdentifierDoubleQuotedState => {}
                State::DocTypeSystemIdentifierSingleQuotedState => {}
                State::AfterDocTypeSystemIdentifiedState => {}
                State::BogusDocTypeState => {
                    let c = self.stream.read_char();
                    match c {
                        Some('>') => {
                            emit_current_token!(self);
                            self.state = State::DataState;
                        }
                        Some(CHAR_NUL) => self.parse_error(ParserError::UnexpectedNullCharacter),
                        None => {
                            emit_current_token!(self);
                            self.state = State::DataState;
                        }
                        _ => {
                            // ignore
                        }
                    }
                }
                State::CDataSectionState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(']') => {
                            self.state = State::CDataSectionBracketState;
                        }
                        None => {
                            self.parse_error(ParserError::EofInCdata);
                            emit_current_token!(self);
                            self.state = State::DataState;
                        },
                        _ => self.consume(c.unwrap()),
                    }
                },
                State::CDataSectionBracketState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(']') => self.state = State::CDataSectionEndState,
                        _ => {
                            self.consume(']');
                            self.stream.unread();
                            self.state = State::CDataSectionState;
                        }
                    }
                },
                State::CDataSectionEndState => {
                    let c = self.stream.read_char();
                    match c {
                        Some(']') => self.state = State::CDataSectionEndState,
                        Some('>') => self.state = State::DataState,
                        _ => {
                            self.consume(']');
                            self.stream.unread();
                            self.state = State::CDataSectionState;
                        }
                    }
                }
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
    pub(crate) fn consume_string(&mut self, s: &str) {
        // Add c to the current token data
        for c in s.chars() {
            self.consumed.push(c)
        }
    }

    fn is_appropriate_end_token(&self, end_token: &Vec<char>) -> bool {
        let s: String = end_token.iter().collect();
        self.last_start_token == s
    }

    // Return the consumed string as a String
    pub fn get_consumed_str(&self) -> String {
        return self.consumed.iter().collect();
    }

    // Returns true if there is anything in the consume buffer
    pub fn has_consumed_data(&self) -> bool {
        return self.consumed.len() > 0;
    }

    // Clears the current consume buffer
    pub(crate) fn clear_consume_buffer(&mut self) {
        self.consumed.clear()
    }

    pub fn get_errors(&self) -> &Vec<ParseError> {
        &self.errors
    }

    // Creates a parser log error message
    pub(crate) fn parse_error(&mut self, error: ParserError) {
        // Hack: when encountering eof, we need to have the previous position, not the current one.
        let mut pos = self.stream.get_position(self.stream.position.offset - 1);
        match error {
            ParserError::EofBeforeTagName |
            ParserError::EofInCdata |
            ParserError::EofInComment |
            ParserError::EofInDoctype |
            ParserError::EofInScriptHtmlCommentLikeText |
            ParserError::EofInTag => {
                pos = self.stream.get_position(self.stream.position.offset);
            }
            _ => {}
        }

        // Add to parse log
        self.errors.push(ParseError{
            message: error.as_str().to_string(),
            line: pos.line,
            col: pos.col,
            offset: pos.offset,
        });
    }

    // Set is_closing_tag in current token
    fn set_is_closing_in_current_token(&mut self, is_closing: bool) {
        match &mut self.current_token.as_mut().unwrap() {
            Token::StartTagToken { is_self_closing, .. } => {
                *is_self_closing = is_closing;
            }
            _ => {
                // @TODO: this was not a start tag token
            }
        }

        self.clear_consume_buffer();
    }

    // Adds a new attribute to the current token
    fn set_add_attribute_to_current_token(&mut self, name: String, value: String) {
        match &mut self.current_token.as_mut().unwrap() {
            Token::StartTagToken { attributes, .. } => {
                attributes.push(
                    (name.clone(), value.clone())
                );
            }
            _ => {
                // @TODO: this was not a start tag token
            }
        }

        self.current_attr_name.clear()
    }

    // Sets the given name into the current token
    fn set_name_in_current_token(&mut self, new_name: String) {
        match &mut self.current_token.as_mut().unwrap() {
            Token::StartTagToken { name, .. } => {
                *name = new_name.clone();
            },
            Token::EndTagToken { name, .. } => {
                *name = new_name.clone();
            },
            _ => panic!("trying to set the name of a non start/end tag token")
        }
    }

    // // Pushes a token to the stack
    // fn push_token_to_queue(&mut self, token: &Token) {
    //
    //     // Save the start token name if we are pushing it. This helps us in detecting matching tags.
    //     match token {
    //         Token::StartTagToken { name, .. } => {
    //             self.last_start_token = String::from(name);
    //         },
    //         _ => {}
    //     }
    //
    //     self.token_queue.push(token.clone());
    // }

    // // Pushes the current configured token onto the token stack, and clears the current token
    // fn push_current_token_to_queue(&mut self) {
    //     // If we are pushing a start token, remember the name for later end-tag matching use
    //     if self.current_token.is_some() {
    //         match self.current_token.clone().unwrap() {
    //             Token::StartTagToken { name, .. } => {
    //                 self.last_start_token = name;
    //             },
    //             _ => {}
    //         }
    //     }
    //
    //     // If there is any consumed data, emit this first as a text token
    //     if self.has_consumed_data() {
    //         self.push_token_to_queue(&Token::TextToken{
    //             value: self.get_consumed_str(),
    //         });
    //         self.clear_consume_buffer();
    //     }
    //
    //     // We are cloning the current token before we send it to the token_queue. This might be inefficient.
    //     self.token_queue.push(self.current_token.clone().unwrap());
    //     self.current_token = None;
    // }

    // This function checks to see if there is already an attribute name like the one in current_attr_name.
    fn check_if_attr_already_exists(&mut self) {
        self.ignore_attribute = false;

        match &mut self.current_token {
            Some(Token::StartTagToken { attributes, .. }) => {
                for (name, ..) in attributes {
                    if name.clone() == self.current_attr_name {
                        self.ignore_attribute = true;
                        return;
                    }
                }
            }
            _ => {}
        }
    }
}