// The different tokens types that can be emitted by the tokenizer
#[derive(Debug, PartialEq)]
pub enum TokenType {
    DocTypeToken,
    StartTagToken,
    EndTagToken,
    CommentToken,
    TextToken,
    EofToken,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Attribute {
    pub name: String,
    pub value: String,
}

// The different token structures that can be emitted by the tokenizer
#[derive(Clone, PartialEq)]
pub enum Token {
    DocTypeToken {
        name: Option<String>,
        force_quirks: bool,
        pub_identifier: Option<String>,
        sys_identifier: Option<String>,
    },
    StartTagToken {
        name: String,
        is_self_closing: bool,
        attributes: Vec<Attribute>,
    },
    EndTagToken {
        name: String,
    },
    CommentToken {
        value: String,
    },
    TextToken {
        value: String,
    },
    EofToken,
}

impl Token {
    pub fn is_eof(&self) -> bool {
        if let Token::EofToken = self {
            true
        } else {
            false
        }
    }

    pub fn is_empty_or_white(&self) -> bool {
        if let Token::TextToken { value } = self {
            value.trim().is_empty()
        } else {
            false
        }
    }
}

// Each token can be displayed as a string
impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Token::DocTypeToken {
                name,
                force_quirks,
                pub_identifier,
                sys_identifier,
            } => {
                let mut result = format!("<!DOCTYPE {}", name.clone().unwrap_or("".to_string()));
                if *force_quirks {
                    result.push_str(" FORCE_QUIRKS!");
                }
                if let Some(pub_id) = pub_identifier {
                    result.push_str(&format!(" {}", pub_id));
                }
                if let Some(sys_id) = sys_identifier {
                    result.push_str(&format!(" {}", sys_id));
                }
                result.push_str(" />");
                write!(f, "{}", result)
            }
            Token::CommentToken { value } => write!(f, "Comment[<!-- {} -->]", value),
            Token::TextToken { value } => write!(f, "Text[{}]", value),
            Token::StartTagToken {
                name,
                is_self_closing,
                attributes,
            } => {
                let mut result = format!("<{}", name);
                for attr in attributes.iter() {
                    result.push_str(&format!(" {}=\"{}\"", attr.name, attr.value));
                }
                if *is_self_closing {
                    result.push_str(" /");
                }
                result.push('>');
                write!(f, "StartTag[{}]", result)
            }
            Token::EndTagToken { name } => write!(f, "EndTag[</{}>]", name),
            Token::EofToken => write!(f, "EOF"),
        }
    }
}

pub trait TokenTrait {
    // Return the token type of the given token
    fn type_of(&self) -> TokenType;
}

// Each token implements the TokenTrait and has a type_of that will return the tokentype.
impl TokenTrait for Token {
    fn type_of(&self) -> TokenType {
        match self {
            Token::DocTypeToken { .. } => TokenType::DocTypeToken,
            Token::StartTagToken { .. } => TokenType::StartTagToken,
            Token::EndTagToken { .. } => TokenType::EndTagToken,
            Token::CommentToken { .. } => TokenType::CommentToken,
            Token::TextToken { .. } => TokenType::TextToken,
            Token::EofToken => TokenType::EofToken,
        }
    }
}
