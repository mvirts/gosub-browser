use crate::html5_parser::input_stream::InputStream;
use crate::html5_parser::node::Node;
use crate::html5_parser::token::{Token, TokenTrait, TokenType};
use crate::html5_parser::tokenizer::Tokenizer;

// Insertion modes as defined in 13.2.4.1
#[derive(Debug, Copy, Clone)]
enum InsertionMode {
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    InHeadNoscript,
    AfterHead,
    InBody,
    Text,
    InTable,
    InTableText,
    InCaption,
    InColumnGroup,
    InTableBody,
    InRow,
    InCell,
    InSelect,
    InSelectInTable,
    InTemplate,
    AfterBody,
    InFrameset,
    AfterFrameset,
    AfterAfterBody,
    AfterAfterFrameset
}

pub struct Html5Parser<'a> {
    tokenizer: Tokenizer<'a>,           // tokenizer object
    insertion_mode: InsertionMode,      // current insertion mode
    parser_cannot_change_mode: bool,    // ??
    current_token: Token                // Current token from the tokenizer
}

pub enum QuirksMode {
    Quirks,
    LimitedQuirks,
    NoQuirks,
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum DocumentType {
    HTML,
    IframeSrcDoc,
}

pub struct Document<'a> {
    pub root: Node<'a>,
    pub doctype: DocumentType,
    pub quirks_mode: QuirksMode,
}

impl<'a> Html5Parser<'a> {
    // Creates a new parser object with the given input stream
    pub fn new(stream: &'a mut InputStream) -> Self {
        Html5Parser {
            tokenizer: Tokenizer::new(stream, None),
            insertion_mode: InsertionMode::Initial,
            parser_cannot_change_mode: false,
            current_token: Token::EofToken,
        }
    }

    // Parses the input stream into a Node tree
    pub fn parse(&mut self) -> Document {
        let mut document = Document {
            root: Node::new("root"),
            doctype: DocumentType::HTML,
            quirks_mode: QuirksMode::NoQuirks,
        };

        loop {
            self.current_token = self.tokenizer.next_token();
            if self.current_token.is_eof() {
                break;
            }

            match self.insertion_mode {
                InsertionMode::Initial => {
                    match &self.current_token {
                        Token::CommentToken { .. } => {
                            document.root.add_child(Node::new("comment"))
                        }
                        Token::DocTypeToken { name, pub_identifier, sys_identifier, force_quirks } => {
                            if name.is_some() && name.as_ref().unwrap() != "html" ||
                                pub_identifier.is_some() ||
                                (sys_identifier.is_some() && sys_identifier.as_ref().unwrap() != "about:legacy-compat")
                            {
                                panic!("parse error: invalid doctype {}/{}", self.tokenizer.get_position().line, self.tokenizer.get_position().col);
                            }

                            document.root.add_child(Node::new("doctype: "));

                            if document.doctype != DocumentType::IframeSrcDoc && self.parser_cannot_change_mode {
                                document.quirks_mode = self.identify_quirks_mode(name.clone(), pub_identifier.clone(), sys_identifier.clone(), *force_quirks);
                            }

                            self.insertion_mode = InsertionMode::BeforeHtml;
                        },
                        _ => {
                            if self.current_token.type_of() == TokenType::TextToken && self.current_token.is_empty_or_white() {
                                // ignore token
                            }

                            if document.doctype != DocumentType::IframeSrcDoc {
                                panic!("parse error: not an iframe doc src {}/{}", self.tokenizer.get_position().line, self.tokenizer.get_position().col);
                            }

                            if self.parser_cannot_change_mode {
                                document.quirks_mode = QuirksMode::Quirks;
                            }

                            self.insertion_mode = InsertionMode::BeforeHtml;
                        }
                    }
                },
                InsertionMode::BeforeHtml => {

                }
                InsertionMode::BeforeHead => {}
                InsertionMode::InHead => {}
                InsertionMode::InHeadNoscript => {}
                InsertionMode::AfterHead => {}
                InsertionMode::InBody => {}
                InsertionMode::Text => {}
                InsertionMode::InTable => {}
                InsertionMode::InTableText => {}
                InsertionMode::InCaption => {}
                InsertionMode::InColumnGroup => {}
                InsertionMode::InTableBody => {}
                InsertionMode::InRow => {}
                InsertionMode::InCell => {}
                InsertionMode::InSelect => {}
                InsertionMode::InSelectInTable => {}
                InsertionMode::InTemplate => {}
                InsertionMode::AfterBody => {}
                InsertionMode::InFrameset => {}
                InsertionMode::AfterFrameset => {}
                InsertionMode::AfterAfterBody => {}
                InsertionMode::AfterAfterFrameset => {}
            }

            for error in &self.tokenizer.errors {
                println!("({}/{}): {}", error.line, error.col, error.message);
            }
        }

        return document;
    }

    // returns the correct quirk mode for the given doctype
    fn identify_quirks_mode(&self, name: Option<String>, pub_identifer: Option<String>, sys_identifier: Option<String>, force_quirks: bool) -> QuirksMode
    {
        if force_quirks || name.unwrap_or("".to_string()).to_uppercase() != "HTML" {
            return QuirksMode::Quirks;
        }

        if pub_identifer.is_some() {
            let pub_id = pub_identifer.unwrap().to_lowercase();
            if QUIRKS_PUB_IDENTIFIER_EQ.contains(&pub_id.as_str()) {
                return QuirksMode::Quirks;
            }
            if QUIRKS_PUB_IDENTIFIER_PREFIX.iter().any(|&prefix| pub_id.as_str().starts_with(&prefix)) {
                return QuirksMode::Quirks;
            }

            if sys_identifier.is_none() {
                if QUIRKS_PUB_IDENTIFIER_PREFIX_MISSING_SYS.iter().any(|&prefix| pub_id.as_str().starts_with(&prefix)) {
                    return QuirksMode::Quirks;
                }
            }

            if LIMITED_QUIRKS_PUB_IDENTIFIER_PREFIX.iter().any(|&prefix| pub_id.as_str().starts_with(&prefix)) {
                return QuirksMode::LimitedQuirks;
            }

            if sys_identifier.is_some() {
                if LIMITED_QUIRKS_PUB_IDENTIFIER_PREFIX.iter().any(|&prefix| pub_id.as_str().starts_with(&prefix)) {
                    return QuirksMode::LimitedQuirks;
                }
            }
        }

        if sys_identifier.is_some() {
            let sys_id = sys_identifier.unwrap().to_lowercase();
            if QUIRKS_SYS_IDENTIFIER_EQ.iter().any(|&prefix| sys_id.as_str().starts_with(&prefix)) {
                return QuirksMode::Quirks;
            }
        }

        return QuirksMode::NoQuirks;
    }
}

static QUIRKS_PUB_IDENTIFIER_EQ: &'static [&'static str] = &[
    "-//W3O//DTD W3 HTML Strict 3.0//EN//",
    "-/W3C/DTD HTML 4.0 Transitional/EN",
    "HTML"
];

static QUIRKS_PUB_IDENTIFIER_PREFIX: &'static [&'static str] = &[
    "+//Silmaril//dtd html Pro v0r11 19970101//",
    "-//AS//DTD HTML 3.0 asWedit + extensions//",
    "-//AdvaSoft Ltd//DTD HTML 3.0 asWedit + extensions//",
    "-//IETF//DTD HTML 2.0 Level 1//",
    "-//IETF//DTD HTML 2.0 Level 2//",
    "-//IETF//DTD HTML 2.0 Strict Level 1//",
    "-//IETF//DTD HTML 2.0 Strict Level 2//",
    "-//IETF//DTD HTML 2.0 Strict//",
    "-//IETF//DTD HTML 2.0//",
    "-//IETF//DTD HTML 2.1E//",
    "-//IETF//DTD HTML 3.0//",
    "-//IETF//DTD HTML 3.2 Final//",
    "-//IETF//DTD HTML 3.2//",
    "-//IETF//DTD HTML 3//",
    "-//IETF//DTD HTML Level 0//",
    "-//IETF//DTD HTML Level 1//",
    "-//IETF//DTD HTML Level 2//",
    "-//IETF//DTD HTML Level 3//",
    "-//IETF//DTD HTML Strict Level 0//",
    "-//IETF//DTD HTML Strict Level 1//",
    "-//IETF//DTD HTML Strict Level 2//",
    "-//IETF//DTD HTML Strict Level 3//",
    "-//IETF//DTD HTML Strict//",
    "-//IETF//DTD HTML//",
    "-//Metrius//DTD Metrius Presentational//",
    "-//Microsoft//DTD Internet Explorer 2.0 HTML Strict//",
    "-//Microsoft//DTD Internet Explorer 2.0 HTML//",
    "-//Microsoft//DTD Internet Explorer 2.0 Tables//",
    "-//Microsoft//DTD Internet Explorer 3.0 HTML Strict//",
    "-//Microsoft//DTD Internet Explorer 3.0 HTML//",
    "-//Microsoft//DTD Internet Explorer 3.0 Tables//",
    "-//Netscape Comm. Corp.//DTD HTML//",
    "-//Netscape Comm. Corp.//DTD Strict HTML//",
    "-//O'Reilly and Associates//DTD HTML 2.0//",
    "-//O'Reilly and Associates//DTD HTML Extended 1.0//",
    "-//O'Reilly and Associates//DTD HTML Extended Relaxed 1.0//",
    "-//SQ//DTD HTML 2.0 HoTMetaL + extensions//",
    "-//SoftQuad Software//DTD HoTMetaL PRO 6.0::19990601::extensions to HTML 4.0//",
    "-//SoftQuad//DTD HoTMetaL PRO 4.0::19971010::extensions to HTML 4.0//",
    "-//Spyglass//DTD HTML 2.0 Extended//",
    "-//Sun Microsystems Corp.//DTD HotJava HTML//",
    "-//Sun Microsystems Corp.//DTD HotJava Strict HTML//",
    "-//W3C//DTD HTML 3 1995-03-24//",
    "-//W3C//DTD HTML 3.2 Draft//",
    "-//W3C//DTD HTML 3.2 Final//",
    "-//W3C//DTD HTML 3.2//",
    "-//W3C//DTD HTML 3.2S Draft//",
    "-//W3C//DTD HTML 4.0 Frameset//",
    "-//W3C//DTD HTML 4.0 Transitional//",
    "-//W3C//DTD HTML Experimental 19960712//",
    "-//W3C//DTD HTML Experimental 970421//",
    "-//W3C//DTD W3 HTML//",
    "-//W3O//DTD W3 HTML 3.0//",
    "-//WebTechs//DTD Mozilla HTML 2.0//",
    "-//WebTechs//DTD Mozilla HTML//",
];

static QUIRKS_PUB_IDENTIFIER_PREFIX_MISSING_SYS: &'static [&'static str] = &[
    "-//W3C//DTD HTML 4.01 Frameset//",
    "-//W3C//DTD HTML 4.01 Transitional//",
];

static QUIRKS_SYS_IDENTIFIER_EQ: &'static [&'static str] = &[
    "http://www.ibm.com/data/dtd/v11/ibmxhtml1-transitional.dtd"
];

static LIMITED_QUIRKS_PUB_IDENTIFIER_PREFIX: &'static [&'static str] = &[
    "-//W3C//DTD XHTML 1.0 Frameset//",
    "-//W3C//DTD XHTML 1.0 Transitional//"
];

static LIMITED_QUIRKS_PUB_IDENTIFIER_PREFIX_NOT_MISSING_SYS: &'static [&'static str] = &[
    "-//W3C//DTD HTML 4.01 Frameset//",
    "-//W3C//DTD HTML 4.01 Transitional//",
];