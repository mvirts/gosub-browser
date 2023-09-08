mod quirks;
pub mod document;

// ------------------------------------------------------------

use crate::html5_parser::input_stream::InputStream;
use crate::html5_parser::node::{Node, NodeData};
use crate::html5_parser::parser::document::{Document, DocumentType};
use crate::html5_parser::parser::quirks::QuirksMode;
use crate::html5_parser::tokenizer::{CHAR_NUL, Tokenizer};
use crate::html5_parser::tokenizer::token::{Attribute, Token};

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

// Additional extensions to the Vec type so we can do some stack operations
trait VecExtensions<T> {
    fn pop_until<F>(&mut self, f: F) where F: FnMut(&T) -> bool;
    fn pop_check<F>(&mut self, f: F) -> bool where F: FnMut(&T) -> bool;
}

impl VecExtensions<usize> for Vec<usize> {
    fn pop_until<F>(&mut self, mut f: F)
    where
        F: FnMut(&usize) -> bool,
    {
        while let Some(top) = self.last() {
            if f(top) {
                break;
            }
            self.pop();
        }
    }

    fn pop_check<F>(&mut self, mut f: F) -> bool
    where
        F: FnMut(&usize) -> bool,
    {
        match self.pop() {
            Some(popped_value) => f(&popped_value),
            None => false,
        }
    }
}

macro_rules! pop_until {
    ($self:expr, $name:expr) => {
        $self.open_elements.pop_until(|node_id| $self.document.get_node_by_id(*node_id).expect("node not found").name == $name);
    };
}

macro_rules! pop_until_any {
    ($self:expr, $arr:expr) => {
        $self.open_elements.pop_until(|node_id| $arr.contains(&$self.document.get_node_by_id(*node_id).expect("node not found").name.as_str()));
    };
}

// Pops the last element from the open elements, and panics if it is not $name
macro_rules! pop_check {
    ($self:expr, $name:expr) => {
        if ! $self.open_elements.pop_check(|node_id| $self.document.get_node_by_id(*node_id).expect("node not found").name == $name) {
            panic!("$name tag should be popped from open elements");
        }
    };
}

// Checks if the last element on the open elements is $name, and panics if not
macro_rules! check_last_element {
    ($self:expr, $name:expr) => {
        let node_id = $self.open_elements.last().unwrap_or(&0);
        if $self.document.get_node_by_id(*node_id).expect("node not found").name != "$name" {
            panic!("$name tag should be last element in open elements");
        }
    };
}

macro_rules! open_elements_get {
    ($self:expr, $idx:expr) => {
        $self.document.get_node_by_id($self.open_elements[$idx]).expect("Open element not found")
    };
}

// macro_rules! open_elements_find {
//     ($self:expr, $name:expr) => {
//         $self.open_elements.iter().rev().find(|node_id| $self.document.get_node_by_id(**node_id).unwrap_or(&NULL_NODE).name == $name)
//     };
// }

macro_rules! open_elements_has {
    ($self:expr, $name:expr) => {
        $self.open_elements.iter().rev().any(|node_id| $self.document.get_node_by_id(*node_id).expect("node not found").name == $name)
    };
}

macro_rules! current_node {
    ($self:expr) => {
        {
            let current_node_idx = $self.open_elements.last().unwrap_or(&0);
            $self.document.get_node_by_id(*current_node_idx).expect("Current node not found")
        }
    };
}

macro_rules! current_node_mut {
    ($self:expr) => {
        {
            let current_node_idx = $self.open_elements.last().unwrap_or(&0);
            $self.document.get_mut_node_by_id(*current_node_idx).expect("Current node not found")
        }
    };
}

macro_rules! open_elements_by_index {
    ($self:expr, $idx:expr) => {
        {
            let node_idx = $self.open_elements.get($idx).unwrap_or(&0);
            $self.document.get_node_by_id(*node_idx).expect("Current node not found")
        }
    };
}

// Active formatting elements, which could be a regular node(id), or a marker
enum ActiveElement {
    Node(usize),
    Marker,
}

// The main parser object
pub struct Html5Parser<'a> {
    tokenizer: Tokenizer<'a>,                       // tokenizer object
    insertion_mode: InsertionMode,                  // current insertion mode
    original_insertion_mode: InsertionMode,         // original insertion mode (used for text mode)
    template_insertion_mode: Vec<InsertionMode>,    // template insertion mode stack
    parser_cannot_change_mode: bool,                // ??
    current_token: Token,                           // Current token from the tokenizer
    reprocess_token: bool,                          // If true, the current token should be processed again
    open_elements: Vec<usize>,                      // Stack of open elements
    head_element: Option<usize>,                    // Current head element
    form_element: Option<usize>,                    // Current form element
    scripting_enabled: bool,                        // If true, scripting is enabled
    frameset_ok: bool,                              // if true, we can insert a frameset
    foster_parenting: bool,                         // Foster parenting flag
    script_already_started: bool,                   // If true, the script engine has already started
    pending_table_character_tokens: Vec<char>,      // Pending table character tokens
    ack_self_closing: bool,                         // Acknowledge self closing tags
    active_formatting_elements: Vec<ActiveElement>, // List of active formatting elements or markers
    is_fragment_case: bool,                         // Is the current parsing a fragment case
    document: &'a mut Document,                     // A reference to the document we are parsing
}

// Defines the scopes for in_scope()
enum Scope {
    Regular,
    ListItem,
    Button,
    Table,
    Select,
}

impl<'a> Html5Parser<'a> {
    // Creates a new parser object with the given input stream
    pub fn new(stream: &'a mut InputStream, document: &'a mut Document) -> Self {
        Html5Parser {
            tokenizer: Tokenizer::new(stream, None),
            insertion_mode: InsertionMode::Initial,
            original_insertion_mode: InsertionMode::Initial,
            template_insertion_mode: vec![],
            parser_cannot_change_mode: false,
            current_token: Token::EofToken,
            reprocess_token: false,
            open_elements: Vec::new(),
            head_element: None,
            form_element: None,
            scripting_enabled: true,
            frameset_ok: true,
            foster_parenting: false,
            script_already_started: false,
            pending_table_character_tokens: vec![],
            ack_self_closing: false,
            active_formatting_elements: vec![],
            is_fragment_case: false,
            document: document,
        }
    }

    // Parses the input stream into a Node tree
    pub fn parse(&mut self) {
        loop {
            // If reprocess_token is true, we should process the same token again
            if !self.reprocess_token {
                self.current_token = self.tokenizer.next_token();
            }
            self.reprocess_token = false;
            if self.current_token.is_eof() {
                break;
            }

            println!("Token: {}", self.current_token);

            match self.insertion_mode {
                InsertionMode::Initial => {
                    match &self.current_token {
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            // ignore token
                        },
                        Token::CommentToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        }
                        Token::DocTypeToken { name, pub_identifier, sys_identifier, force_quirks } => {
                            if name.is_some() && name.as_ref().unwrap() != "html" ||
                                pub_identifier.is_some() ||
                                (sys_identifier.is_some() && sys_identifier.as_ref().unwrap() != "about:legacy-compat")
                            {
                                self.parse_error("doctype not allowed in initial insertion mode");
                            }

                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);

                            if self.document.doctype != DocumentType::IframeSrcDoc && self.parser_cannot_change_mode {
                                self.document.quirks_mode = self.identify_quirks_mode(name, pub_identifier.clone(), sys_identifier.clone(), *force_quirks);
                            }

                            self.insertion_mode = InsertionMode::BeforeHtml;
                        },
                        _ => {
                            if self.document.doctype != DocumentType::IframeSrcDoc {
                                self.parse_error("not an iframe doc src");
                            }

                            if self.parser_cannot_change_mode {
                                self.document.quirks_mode = QuirksMode::Quirks;
                            }

                            self.insertion_mode = InsertionMode::BeforeHtml;
                            self.reprocess_token = true;
                        }
                    }
                },
                InsertionMode::BeforeHtml => {
                    let mut anything_else = false;

                    match &self.current_token {
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in before html insertion mode");
                        }
                        Token::CommentToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        }
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            // ignore token
                        }
                        Token::StartTagToken { name, .. } if name == "html" => {
                            let node = self.create_node(&self.current_token);
                            let node_id = self.document.add_node(node, current_node!(self).id);
                            self.open_elements.push(node_id);

                            self.insertion_mode = InsertionMode::BeforeHead;
                        }
                        Token::EndTagToken { name } if name == "head" || name == "body" || name == "html" || name == "br" => {
                            anything_else = true;
                        }
                        Token::EndTagToken { .. } => {
                            self.parse_error("end tag not allowed in before html insertion mode");
                        },
                        _ => {
                            anything_else = true;
                        }
                    }

                    if anything_else {
                        let token = Token::StartTagToken { name: "html".to_string(), is_self_closing: false, attributes: Vec::new() };
                        let node = self.create_node(&token);
                        let node_id = self.document.add_node(node, current_node!(self).id);
                        self.open_elements.push(node_id);

                        self.insertion_mode = InsertionMode::BeforeHead;
                        self.reprocess_token = true;
                    }
                }
                InsertionMode::BeforeHead => {
                    let mut anything_else = false;

                    match &self.current_token {
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            // ignore token
                        },
                        Token::CommentToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        },
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in before head insertion mode");
                        },
                        Token::StartTagToken { name, .. } if name == "html" => {
                            self.handle_in_body();
                        },
                        Token::StartTagToken { name, .. } if name == "head" => {
                            let node = self.create_node(&self.current_token);
                            let node_id = self.document.add_node(node, current_node!(self).id);
                            self.head_element = Some(node_id);

                            self.insertion_mode = InsertionMode::InHead;
                        },
                        Token::EndTagToken { name } if name == "head" || name == "body" || name == "html" || name == "br" => {
                            anything_else = true;
                        }
                        Token::EndTagToken { .. } => {
                            self.parse_error("end tag not allowed in before head insertion mode");
                        },
                        _ => {
                            anything_else = true;
                        }
                    }
                    if anything_else {
                        let token = Token::StartTagToken { name: "head".to_string(), is_self_closing: false, attributes: Vec::new() };
                        let node = self.create_node(&token);
                        let node_id = self.document.add_node(node, current_node!(self).id);
                        self.head_element = Some(node_id);

                        self.insertion_mode = InsertionMode::InHead;
                        self.reprocess_token = true;
                    }
                }
                InsertionMode::InHead => self.handle_in_head(),
                InsertionMode::InHeadNoscript => {
                    let mut anything_else = false;

                    match &self.current_token {
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in 'head no script' insertion mode");
                        },
                        Token::StartTagToken { name, .. } if name == "html" => {
                            self.handle_in_body();
                        },
                        Token::EndTagToken { name, .. } if name == "noscript" => {
                            pop_check!(self, "noscript");
                            check_last_element!(self, "head");

                            self.insertion_mode = InsertionMode::InHead;
                        },
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            self.handle_in_head();
                        },
                        Token::CommentToken { .. } => {
                            self.handle_in_head();
                        },
                        Token::StartTagToken { name, .. } if name == "basefont" || name == "bgsound" || name == "link" || name == "meta" || name == "noframes" || name == "style" => {
                            self.handle_in_head();
                        }
                        Token::EndTagToken { name, .. } if name == "br" => {
                            anything_else = true;
                        }
                        Token::StartTagToken { name, .. } if name == "head" || name == "noscript" => {
                            self.parse_error("head or noscript tag not allowed in after head insertion mode");
                        }
                        Token::EndTagToken { .. } => {
                            self.parse_error("end tag not allowed in after head insertion mode");
                        },
                        _ => {
                            anything_else = true;
                        }
                    }
                    if anything_else {
                        self.parse_error("anything else not allowed in after head insertion mode");

                        pop_check!(self, "noscript");
                        check_last_element!(self, "head");

                        self.insertion_mode = InsertionMode::InHead;
                        self.reprocess_token = true;
                    }
                }
                InsertionMode::AfterHead => {
                    let mut anything_else = false;

                    match &self.current_token {
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            let node = self.create_node(&self.current_token);
                            let node_id = self.document.add_node(node, current_node!(self).id);
                            self.open_elements.push(node_id);
                        },
                        Token::CommentToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            let node_id = self.document.add_node(node, current_node!(self).id);
                            self.open_elements.push(node_id);
                        },
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in after head insertion mode");
                        },
                        Token::StartTagToken { name, .. } if name == "html" => {
                            self.handle_in_body();
                        },
                        Token::StartTagToken { name, .. } if name == "body" => {
                            let node = self.create_node(&self.current_token);
                            let node_id = self.document.add_node(node, current_node!(self).id);
                            self.open_elements.push(node_id);

                            self.frameset_ok = true;
                            self.insertion_mode = InsertionMode::InBody;
                        },
                        Token::StartTagToken { name, .. } if name == "frameset" => {
                            let node = self.create_node(&self.current_token);
                            let node_id = self.document.add_node(node, current_node!(self).id);
                            self.open_elements.push(node_id);

                            self.insertion_mode = InsertionMode::InFrameset;
                        },

                        Token::StartTagToken { name, .. } if ["base", "basefront", "bgsound", "link", "meta", "noframes", "script", "style", "template", "title"].contains(&name.as_str()) => {
                            self.parse_error("invalid start tag in after head insertion mode");

                            if let Some(node_id) = self.head_element {
                                self.open_elements.push(node_id);
                            }

                            self.handle_in_head();

                            // remove the node pointed to by the head element pointer from the stack of open elements (might not be current node at this point)
                        }
                        Token::EndTagToken { name, .. } if name == "template" => {
                            self.handle_in_head();
                        }
                        Token::EndTagToken { name, .. } if name == "body" || name == "html" || name == "br"=> {
                            anything_else = true;
                        }
                        Token::StartTagToken { name, .. } if name == "head" => {
                            self.parse_error("head tag not allowed in after head insertion mode");
                        }
                        Token::EndTagToken { .. }  => {
                            self.parse_error("end tag not allowed in after head insertion mode");
                        }
                        _ => {
                            anything_else = true;
                        }
                    }

                    if anything_else {
                        let token = Token::StartTagToken { name: "body".to_string(), is_self_closing: false, attributes: Vec::new() };
                        let node = self.create_node(&token);
                        self.document.add_node(node, current_node!(self).id);

                        self.insertion_mode = InsertionMode::InBody;
                        self.reprocess_token = true;
                    }
                }
                InsertionMode::InBody => self.handle_in_body(),
                InsertionMode::Text => {
                    match &self.current_token {
                        Token::TextToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            let node_id = self.document.add_node(node, current_node!(self).id);
                            self.open_elements.push(node_id);
                        },
                        Token::EofToken => {
                            self.parse_error("eof not allowed in text insertion mode");

                            if current_node!(self).name == "script" {
                                self.script_already_started = true;
                            }
                            self.open_elements.pop();
                            self.insertion_mode = self.original_insertion_mode;
                        },
                        Token::EndTagToken { name, .. } if name == "script" => {
                            // @TODO: do script stuff!!!!
                        }
                        _ => {
                            self.open_elements.pop();
                            self.insertion_mode = self.original_insertion_mode;
                        }
                    }
                }
                InsertionMode::InTable => self.handle_in_table(),
                InsertionMode::InTableText => {
                    match &self.current_token {
                        Token::TextToken { value, .. } => {
                            for c in value.chars() {
                                if c == CHAR_NUL {
                                    self.parse_error("null character not allowed in in table insertion mode");
                                } else {
                                    self.pending_table_character_tokens.push(c);
                                }
                            }
                        }
                        _ => {
                            // @TODO: this needs to check if there are any non-whitespaces, if so then
                            // reprocess using anything_else in "in_table"
                            self.flush_pending_table_character_tokens();
                            self.insertion_mode = self.original_insertion_mode;
                            self.reprocess_token = true;
                        }
                    }
                }
                InsertionMode::InCaption => {
                    let process_incaption_body;

                    match &self.current_token {
                        Token::EndTagToken { name, .. } if name == "caption" => {
                            process_incaption_body = true;
                        }
                        Token::StartTagToken { name, .. } if ["caption", "col", "colgroup", "tbody", "td", "tfoot", "th", "thead", "tr"].contains(&name.as_str()) => {
                            process_incaption_body = true;
                            self.reprocess_token = true;
                        }
                        Token::EndTagToken { name, .. } if name == "table" => {
                            process_incaption_body = true;
                            self.reprocess_token = true;
                        }
                        _ => {
                            // process using rules like inbody insertion mode
                            continue;
                        }
                    }

                    if process_incaption_body {
                        if ! open_elements_has!(self, "caption") {
                            self.parse_error("caption end tag not allowed in in caption insertion mode");
                            continue;
                        }

                        self.generate_all_implied_end_tags(None, false);

                        if current_node!(self).name != "caption" {
                            self.parse_error("caption end tag not at top of stack");
                            continue;
                        }

                        pop_until!(self, "caption");
                        self.clear_active_formatting_elements_until_marker();

                        self.insertion_mode = InsertionMode::InTable;
                    }
                }
                InsertionMode::InColumnGroup => {
                    let mut anything_else = false;

                    match &self.current_token {
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            let node = self.create_node(&self.current_token);
                            let node_id = self.document.add_node(node, current_node!(self).id);
                            self.open_elements.push(node_id);
                        },
                        Token::CommentToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            let node_id = self.document.add_node(node, current_node!(self).id);
                            self.open_elements.push(node_id);
                        },
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in column group insertion mode");
                        },
                        Token::StartTagToken { name, .. } if name == "html" => {
                            self.handle_in_body();
                        },
                        Token::StartTagToken { name, is_self_closing, .. } if name == "col" => {
                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);

                            self.open_elements.pop();

                            if *is_self_closing {
                                self.acknowledge_self_closing_tag(&self.current_token.clone());
                            }
                        },
                        Token::StartTagToken { name, .. } if name == "frameset" => {
                            let node = self.create_node(&self.current_token);
                            let node_id = self.document.add_node(node, current_node!(self).id);
                            self.open_elements.push(node_id);

                            self.insertion_mode = InsertionMode::InFrameset;
                        },

                        Token::StartTagToken { name, .. } if ["base", "basefront", "bgsound", "link", "meta", "noframes", "script", "style", "template", "title"].contains(&name.as_str()) => {
                            self.parse_error("invalid start tag in after head insertion mode");

                            if let Some(ref value) = self.head_element {
                                self.open_elements.push(value.clone());
                            }

                            self.handle_in_head();

                            // remove the node pointed to by the head element pointer from the stack of open elements (might not be current node at this point)
                        }
                        Token::EndTagToken { name, .. } if name == "template" => {
                            self.handle_in_head();
                        }
                        Token::EndTagToken { name, .. } if name == "body" || name == "html" || name == "br"=> {
                            anything_else = true;
                        }
                        Token::StartTagToken { name, .. } if name == "head" => {
                            self.parse_error("head tag not allowed in after head insertion mode");
                        }
                        Token::EndTagToken { .. }  => {
                            self.parse_error("end tag not allowed in after head insertion mode");
                        }
                        _ => {
                            anything_else = true;
                        }
                    }

                    if anything_else {
                        let token = Token::StartTagToken { name: "body".to_string(), is_self_closing: false, attributes: Vec::new() };
                        let node = self.create_node(&token);
                        self.document.add_node(node, current_node!(self).id);

                        self.insertion_mode = InsertionMode::InBody;
                        self.reprocess_token = true;
                    }
                }
                InsertionMode::InTableBody => {
                    match &self.current_token {
                        Token::StartTagToken { name, .. } if name == "tr" => {
                            self.clear_stack_back_to_table_context();

                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);

                            self.insertion_mode = InsertionMode::InRow;
                        },
                        Token::StartTagToken { name, .. } if name == "th" || name == "td" => {
                            self.parse_error("th or td tag not allowed in in table body insertion mode");

                            self.clear_stack_back_to_table_context();

                            let token = Token::StartTagToken { name: "tr".to_string(), is_self_closing: false, attributes: Vec::new() };
                            let node = self.create_node(&token);
                            self.document.add_node(node, current_node!(self).id);

                            self.insertion_mode = InsertionMode::InRow;
                            self.reprocess_token = true;
                        },
                        Token::StartTagToken { name, .. } if name == "tbody" || name == "tfoot" || name == "thead" => {

                            if ! self.in_scope(name, Scope::Table) {
                                self.parse_error("tbody, tfoot or thead tag not allowed in in table body insertion mode");
                                continue;
                            }

                            self.clear_stack_back_to_table_context();
                            self.open_elements.pop();

                            self.insertion_mode = InsertionMode::InTable;
                        },
                        Token::StartTagToken { name, .. } if ["caption", "col", "colgroup", "tbody", "tfoot", "thead"].contains(&name.as_str()) => {
                            if ! self.in_scope("tbody", Scope::Table) && ! self.in_scope("tfoot", Scope::Table) && ! self.in_scope("thead", Scope::Table) {
                                self.parse_error("caption, col, colgroup, tbody, tfoot or thead tag not allowed in in table body insertion mode");
                                continue;
                            }

                            self.clear_stack_back_to_table_context();
                            self.open_elements.pop();

                            self.insertion_mode = InsertionMode::InTable;
                        }
                        Token::EndTagToken { name, .. } if name == "table" => {
                            if ! self.in_scope("tbody", Scope::Table) && ! self.in_scope("tfoot", Scope::Table) && ! self.in_scope("thead", Scope::Table) {
                                self.parse_error("caption, col, colgroup, tbody, tfoot or thead tag not allowed in in table body insertion mode");
                                continue;
                            }

                            self.clear_stack_back_to_table_context();
                            self.open_elements.pop();

                            self.insertion_mode = InsertionMode::InTable;
                        }
                        Token::EndTagToken { name, .. } if ["body", "caption", "col", "colgroup", "html", "td", "th", "tr"].contains(&name.as_str()) => {
                            self.parse_error("end tag not allowed in in table body insertion mode");
                        }
                        _ => {
                            self.handle_in_table();
                        }
                    }
                }
                InsertionMode::InRow => {
                    match &self.current_token {
                        Token::StartTagToken { name, .. } if name == "th" || name == "td" => {
                            self.parse_error("th or td tag not allowed in in table body insertion mode");

                            self.clear_stack_back_to_table_row_context();

                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);

                            self.insertion_mode = InsertionMode::InCell;
                            self.add_marker();
                        },
                        Token::EndTagToken { name, .. } if name == "tr" => {
                            if ! self.in_scope("tr", Scope::Table) {
                                self.parse_error("tr tag not allowed in in row insertion mode");
                                continue;
                            }

                            self.clear_stack_back_to_table_row_context();
                            pop_check!(self, "tr");

                            self.insertion_mode = InsertionMode::InTableBody;
                        }
                        Token::StartTagToken { name, .. } if ["caption", "col", "colgroup", "tbody", "tfoot", "thead", "tr"].contains(&name.as_str()) => {
                            if ! self.in_scope("tr", Scope::Table) {
                                self.parse_error("caption, col, colgroup, tbody, tfoot or thead tag not allowed in in row insertion mode");
                                continue;
                            }

                            self.clear_stack_back_to_table_row_context();
                            pop_check!(self, "tr");

                            self.insertion_mode = InsertionMode::InTableBody;
                            self.reprocess_token = true;
                        }
                        Token::EndTagToken { name, .. } if name == "tbody" || name == "tfoot" || name == "thead" => {
                            if ! self.in_scope(name, Scope::Table) {
                                self.parse_error("tbody, tfoot or thead tag not allowed in in table body insertion mode");
                                continue;
                            }

                            if ! self.in_scope("tr", Scope::Table) {
                                // ignore
                                continue;
                            }

                            self.clear_stack_back_to_table_row_context();
                            pop_check!(self, "tr");

                            self.insertion_mode = InsertionMode::InTableBody;
                        },
                        _ => {
                            // process in_table insertion mode
                        }
                    }
                }
                InsertionMode::InCell => {
                    // @TODO: Why do i need to clone here and not in other places?
                    let current_token = &self.current_token.clone();
                    match current_token {
                        Token::StartTagToken { name, .. } if name == "th" || name == "td" => {
                            self.parse_error("th or td tag not allowed in in table body insertion mode");

                            self.generate_all_implied_end_tags(None, false);

                            if current_node!(self).name != *name {
                                self.parse_error("current node should be th or td");
                            }

                            pop_until!(self, *name);

                            self.clear_active_formatting_elements_until_marker();

                            self.insertion_mode = InsertionMode::InRow;
                        },
                        Token::StartTagToken { name, .. } if ["caption", "col", "colgroup", "tbody", "td", "tfoot", "th", "thead", "tr"].contains(&name.as_str()) => {
                            if ! self.in_scope("td", Scope::Table) && ! self.in_scope("th", Scope::Table) {
                                self.parse_error("caption, col, colgroup, tbody, tfoot or thead tag not allowed in in cell insertion mode");
                                continue;
                            }

                            self.close_cell();
                            self.reprocess_token = true;
                        }
                        Token::EndTagToken { name, .. } if name == "body" || name == "caption" || name == "col" || name == "colgroup" || name == "html" => {
                            self.parse_error("end tag not allowed in in cell insertion mode");
                        }
                        Token::EndTagToken { name, .. } if name == "tbody" || name == "tfoot" || name == "thead" || name == "tr" => {
                            if ! self.in_scope(name, Scope::Table) {
                                self.parse_error("tbody, tfoot or thead tag not allowed in in table body insertion mode");
                                continue;
                            }

                            self.close_cell();
                            self.reprocess_token = true;
                        },
                        _ => {
                            self.handle_in_body();
                        }
                    }

                }
                InsertionMode::InSelect => {
                    match &self.current_token {
                        Token::TextToken { .. } if self.current_token.is_null() => {
                            self.parse_error("null character not allowed in in select insertion mode");
                            // ignore token
                        },
                        Token::TextToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        },
                        Token::CommentToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        },
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in in select insertion mode");
                            // ignore token
                        },
                        Token::StartTagToken { name, .. } if name == "html" => {
                            self.handle_in_body();
                        },
                        Token::StartTagToken { name, .. } if name == "option" => {
                            if current_node!(self).name == "option" {
                                self.open_elements.pop();
                            }

                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        },
                        Token::StartTagToken { name, is_self_closing, .. } if name == "optgroup" => {
                            if current_node!(self).name == "optgroup" || current_node!(self).name == "option" {
                                self.open_elements.pop();
                            }

                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);

                            self.open_elements.pop();

                            if *is_self_closing {
                                self.acknowledge_self_closing_tag(&self.current_token.clone());
                            }
                        },
                        Token::EndTagToken { name } if name == "optgroup" => {
                            if current_node!(self).name == "option" &&
                                self.open_elements.len() > 1 &&
                                open_elements_get!(self, self.open_elements.len() - 1).name == "optgroup"
                            {
                                self.open_elements.pop();
                            }

                            if current_node!(self).name == "optgroup" {
                                self.open_elements.pop();
                            } else {
                                self.parse_error("optgroup end tag not allowed in in select insertion mode");
                            }
                        },
                        Token::EndTagToken { name } if name == "option" => {
                            if current_node!(self).name == "option" {
                                self.open_elements.pop();
                            } else {
                                self.parse_error("option end tag not allowed in in select insertion mode");
                            }
                        },
                        Token::EndTagToken { name } if name == "select" => {
                            if !self.in_scope("select", Scope::Select) {
                                self.parse_error("select end tag not allowed in in select insertion mode");
                                continue;
                            }

                            pop_until!(self, "select");
                            self.reset_insertion_mode();
                        },
                        Token::StartTagToken { name, .. } if name == "select" => {
                            self.parse_error("select tag not allowed in in select insertion mode");

                            if !self.in_scope("select", Scope::Select) {
                                // ignore token
                                continue;
                            }

                            pop_until!(self, "select");
                            self.reset_insertion_mode();
                        },
                        Token::StartTagToken { name, .. } if name == "input" || name == "keygen" || name == "textarea" => {
                            self.parse_error("input, keygen or textarea tag not allowed in in select insertion mode");

                            if !self.in_scope("select", Scope::Select) {
                                // ignore token
                                continue;
                            }

                            pop_until!(self, "select");
                            self.reset_insertion_mode();
                            self.reprocess_token = true;
                        },

                        Token::StartTagToken { name, .. } if name == "script" || name == "template" => {
                            self.handle_in_head();
                        }
                        Token::EndTagToken { name, .. } if name == "template" => {
                            self.handle_in_head();
                        }
                        Token::EofToken => {
                            self.handle_in_body();
                        }
                        _ => {
                            self.parse_error("anything else not allowed in in select insertion mode");
                            // ignore token
                        }
                    }
                }
                InsertionMode::InSelectInTable => {
                    match &self.current_token {
                        Token::StartTagToken { name, .. } if name == "caption" || name == "table" || name == "tbody" || name == "tfoot" || name == "thead" || name == "tr" || name == "td" || name == "th" => {
                            self.parse_error("caption, table, tbody, tfoot, thead, tr, td or th tag not allowed in in select in table insertion mode");

                            pop_until!(self, "select");
                            self.reset_insertion_mode();
                            self.reprocess_token = true;
                        },
                        Token::EndTagToken { name, .. } if name == "caption" || name == "table" || name == "tbody" || name == "tfoot" || name == "thead" || name == "tr" || name == "td" || name == "th" => {
                            self.parse_error("caption, table, tbody, tfoot, thead, tr, td or th tag not allowed in in select in table insertion mode");

                            if !self.in_scope("select", Scope::Select) {
                                // ignore token
                                continue;
                            }

                            pop_until!(self, "select");
                            self.reset_insertion_mode();
                            self.reprocess_token = true;
                        },
                        _ => {
                            self.handle_in_select();
                        }
                    }
                }
                InsertionMode::InTemplate => {
                    match &self.current_token {
                        Token::TextToken { .. } => {
                            self.handle_in_body();
                        },
                        Token::CommentToken { .. } => {
                            self.handle_in_body();
                        },
                        Token::DocTypeToken { .. } => {
                            self.handle_in_body();
                        },
                        Token::StartTagToken { name, .. } if name == "base" || name == "basefont" || name == "bgsound" || name == "link" || name == "meta" || name == "noframes" || name == "script" || name == "style" || name == "template" || name == "title" => {
                            self.handle_in_head();
                        },
                        Token::EndTagToken { name, .. } if name == "template" => {
                            self.handle_in_head();
                        },
                        Token::StartTagToken { name, .. } if name == "caption" || name == "colgroup" || name == "tbody" || name == "tfoot" || name == "thead" => {
                            self.template_insertion_mode.pop();
                            self.template_insertion_mode.push(InsertionMode::InTable);

                            self.insertion_mode = InsertionMode::InTable;
                            self.reprocess_token = true;
                        },
                        Token::StartTagToken { name, .. } if name == "col" => {
                            self.template_insertion_mode.pop();
                            self.template_insertion_mode.push(InsertionMode::InColumnGroup);

                            self.insertion_mode = InsertionMode::InColumnGroup;
                            self.reprocess_token = true;
                        }
                        Token::StartTagToken { name, .. } if name == "tr" => {
                            self.template_insertion_mode.pop();
                            self.template_insertion_mode.push(InsertionMode::InTableBody);

                            self.insertion_mode = InsertionMode::InTableBody;
                            self.reprocess_token = true;
                        },
                        Token::StartTagToken { name, .. } if name == "td" || name == "th" => {
                            self.template_insertion_mode.pop();
                            self.template_insertion_mode.push(InsertionMode::InRow);

                            self.insertion_mode = InsertionMode::InRow;
                            self.reprocess_token = true;
                        },
                        Token::StartTagToken { .. } => {
                            self.template_insertion_mode.pop();
                            self.template_insertion_mode.push(InsertionMode::InBody);

                            self.insertion_mode = InsertionMode::InBody;
                            self.reprocess_token = true;
                        },
                        Token::EndTagToken { .. }  => {
                            self.parse_error("end tag not allowed in in template insertion mode");
                            // ignore token
                        },
                        Token::EofToken => {
                            if open_elements_has!(self, "template") {
                                self.stop_parsing();
                                continue;
                            }

                            self.parse_error("eof not allowed in in template insertion mode");

                            pop_until!(self, "template");
                            self.clear_active_formatting_elements_until_marker();
                            self.template_insertion_mode.pop();
                            self.reset_insertion_mode();
                            self.reprocess_token = true;
                        },
                    }
                }
                InsertionMode::AfterBody => {
                    match &self.current_token {
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            self.handle_in_body();
                        }
                        Token::CommentToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        },
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in after body insertion mode");
                        },
                        Token::StartTagToken { name, .. } if name == "html" => {
                            self.handle_in_body();
                        }
                        Token::EndTagToken { name, .. } if name == "html" => {
                            // @TODO: something with fragment case
                            self.insertion_mode = InsertionMode::AfterAfterBody;
                        }
                        Token::EofToken => {
                            self.stop_parsing();
                            continue;
                        }
                        _ => {
                            self.parse_error("anything else not allowed in after body insertion mode");
                            self.insertion_mode = InsertionMode::InBody;
                            self.reprocess_token = true;
                        }
                    }
                }
                InsertionMode::InFrameset => {
                    match &self.current_token {
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        }
                        Token::CommentToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        },
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in frameset insertion mode");
                        },
                        Token::StartTagToken { name, .. } if name == "html" => {
                            self.handle_in_body();
                        }
                        Token::StartTagToken { name, .. } if name == "frameset" => {
                            if current_node!(self).name == "html" {
                                self.parse_error("frameset tag not allowed in frameset insertion mode");
                                // ignore token
                                continue;
                            }

                            self.open_elements.pop();

                            if ! self.is_fragment_case && current_node!(self).name != "frameset" {
                                self.insertion_mode = InsertionMode::AfterFrameset;
                            }
                        }
                        Token::StartTagToken { name, is_self_closing, .. } if name == "frame" => {
                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);

                            self.open_elements.pop();

                            if *is_self_closing {
                                self.acknowledge_self_closing_tag(&self.current_token.clone());
                            }
                        }
                        Token::StartTagToken { name, .. } if name == "noframes" => {
                            self.handle_in_head();
                        }
                        Token::EofToken => {
                            if current_node!(self).name != "html" {
                                self.parse_error("eof not allowed in frameset insertion mode");
                            }
                            // @TODO: the current node can be the root html in the fragment case

                            self.stop_parsing();
                            continue;
                        }
                        _ => {
                            self.parse_error("anything else not allowed in frameset insertion mode");
                            // ignore token
                        }
                    }

                }
                InsertionMode::AfterFrameset => {
                    match &self.current_token {
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        }
                        Token::CommentToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        },
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in frameset insertion mode");
                        },
                        Token::StartTagToken { name, .. } if name == "html" => {
                            self.handle_in_body();
                        }
                        Token::EndTagToken { name, .. } if name == "html" => {
                            self.handle_in_head();
                        }
                        Token::EofToken => {
                            // STOP parsing
                        }
                        _ => {
                            self.parse_error("anything else not allowed in after frameset insertion mode");
                            // ignore token
                        }
                    }
                }
                InsertionMode::AfterAfterBody => {
                    match &self.current_token {
                        Token::CommentToken { .. } => {
                            // @TODO: last child of the document object

                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        },
                        Token::DocTypeToken { .. } => {
                            self.handle_in_body();
                        },
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            self.handle_in_body();
                        }
                        Token::StartTagToken { name, .. } if name == "html" => {
                            self.handle_in_body();
                        }
                        Token::EofToken => {
                            // STOP parsing
                        }
                        _ => {
                            self.parse_error("anything else not allowed in after after body insertion mode");
                            self.insertion_mode = InsertionMode::InBody;
                            self.reprocess_token = true;
                        }
                    }
                }
                InsertionMode::AfterAfterFrameset => {
                    match &self.current_token {
                        Token::CommentToken { .. } => {
                            // @TODO: last child of the document object

                            let node = self.create_node(&self.current_token);
                            self.document.add_node(node, current_node!(self).id);
                        },
                        Token::DocTypeToken { .. } => {
                            self.handle_in_body();
                        },
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            self.handle_in_body();
                        }
                        Token::StartTagToken { name, .. } if name == "html" => {
                            self.handle_in_body();
                        }
                        Token::EofToken => {
                            // STOP parsing
                        }
                        Token::StartTagToken { name, .. } if name == "noframes" => {
                            self.handle_in_head();
                        }
                        _ => {
                            self.parse_error("anything else not allowed in after after frameset insertion mode");
                            // ignore token
                        }
                    }
                }
            }

            for error in &self.tokenizer.errors {
                println!("({}/{}): {}", error.line, error.col, error.message);
            }
        }
    }

    // Creates a parse error and halts the parser
    fn parse_error(&self, message: &str) {
        println!("Parse error ({}/{}): {}", self.tokenizer.get_position().line, self.tokenizer.get_position().col, message);
    }

    // Create a new node that is not connected or attached to the document arena
    fn create_node(&self, token: &Token) -> Node {
        let val: String;
        match token {
            Token::DocTypeToken { name, pub_identifier, sys_identifier, force_quirks} => {
                val = format!("doctype[{} {} {} {}]",
                    name.as_deref().unwrap_or(""),
                    pub_identifier.as_deref().unwrap_or(""),
                    sys_identifier.as_deref().unwrap_or(""),
                    force_quirks
                );

                return Node::new_element(val.as_str(), Vec::new());
            }
            Token::StartTagToken { name, is_self_closing, attributes} => {
                val = format!("start_tag[{}, selfclosing: {}]", name, is_self_closing);
                return Node::new_element(val.as_str(), attributes.clone());
            }
            Token::EndTagToken { name } => {
                val = format!("end_tag[{}]", name);
                return Node::new_element(val.as_str(), Vec::new());
            }
            Token::CommentToken { value } => {
                val = format!("comment[{}]", value);
                return Node::new_comment(val.as_str());
            }
            Token::TextToken { value } => {
                val = format!("text[{}]", value);
                return Node::new_text(val.as_str());
            }
            Token::EofToken => {
                panic!("EOF token not allowed");
            }
        }

    }

    fn acknowledge_self_closing_tag(&mut self, _token: &Token) {
        self.ack_self_closing = true;
    }

    fn flush_pending_table_character_tokens(&self) {
        todo!()
    }

    // Clear the active formatting stack until we reach the first marker
    fn clear_active_formatting_elements_until_marker(&mut self) {
        loop {
            let active_elem = self.active_formatting_elements.pop();
            if active_elem.is_none() {
                return;
            }

            if let ActiveElement::Marker = active_elem.unwrap() {
                return;
            }
        }
    }

    // Adds a marker to the active formatting stack
    fn add_marker(&mut self) {
        self.active_formatting_elements.push(ActiveElement::Marker);
    }

    // This function will pop elements off the stack until it reaches the first element that matches
    // our condition (which can be changed with the except and thoroughly parameters)
    fn generate_all_implied_end_tags(&mut self, except: Option<&str>, thoroughly: bool) {
        while self.open_elements.len() > 0 {
            let val = current_node!(self).name.clone();

            if except.is_some() && except.unwrap() == val {
                return;
            }

            if thoroughly && ! ["tbody", "td", "tfoot", "th", "thead", "tr"].contains(&val.as_str()) {
                return;
            }

            if ! ["dd", "dt", "li", "option", "optgroup", "p", "rb", "rp", "rt", "rtc"].contains(&val.as_str()) {
                return;
            }

            self.open_elements.pop();
        }
    }

    // Reset insertion mode based on all kind of rules
    fn reset_insertion_mode(&mut self) {
        let mut last = false;
        let mut idx = self.open_elements.len() - 1;

        loop {
            let node = open_elements_get!(self, idx);
            if idx == 0 {
                last = true;
                // @TODO:
                // if fragment_case {
                //   node = context element !???
                // }
            }

            if node.name == "select" {
                if last {
                    self.insertion_mode = InsertionMode::InSelect;
                    return;
                }

                let mut ancestor_idx = idx;
                loop {
                    if ancestor_idx == 0 {
                        self.insertion_mode = InsertionMode::InSelect;
                        return;
                    }

                    ancestor_idx -= 1;
                    let ancestor = open_elements_get!(self, ancestor_idx);

                    if ancestor.name == "template" {
                        self.insertion_mode = InsertionMode::InSelect;
                        return;
                    }

                    if ancestor.name == "table" {
                        self.insertion_mode = InsertionMode::InSelectInTable;
                        return;
                    }
                }
            }

            if (node.name == "td" || node.name == "th") && !last {
                self.insertion_mode = InsertionMode::InCell;
                return;
            }
            if node.name == "tr" {
                self.insertion_mode = InsertionMode::InRow;
                return;
            }
            if ["tbody", "thead", "tfoot"].iter().any(|&elem| elem == node.name) {
                self.insertion_mode = InsertionMode::InTableBody;
                return;
            }
            if node.name == "caption" {
                self.insertion_mode = InsertionMode::InCaption;
                return;
            }
            if node.name == "colgroup" {
                self.insertion_mode = InsertionMode::InColumnGroup;
                return;
            }
            if node.name == "table" {
                self.insertion_mode = InsertionMode::InTable;
                return;
            }
            if node.name == "template" {
                self.insertion_mode = self.template_insertion_mode.last().unwrap().clone();
                return;
            }
            if node.name == "head" && !last {
                self.insertion_mode = InsertionMode::InHead;
                return;
            }
            if node.name == "body" {
                self.insertion_mode = InsertionMode::InBody;
                return;
            }
            if node.name == "frameset" {
                self.insertion_mode = InsertionMode::InFrameset;
                return;
            }
            if node.name == "html" {
                if self.head_element.is_none() {
                    self.insertion_mode = InsertionMode::BeforeHead;
                    return;
                }
                self.insertion_mode = InsertionMode::AfterHead;
                return;
            }
            if last {
                self.insertion_mode = InsertionMode::InBody;
                return;
            }

            idx -= 1;
        }
    }

    // // Returns the current node id, which is the last node in the open elements stack
    // fn get_current_node_id(&self) -> usize {
    //     match self.open_elements.last() {
    //         Some(node_id) => *node_id,
    //         None => 0,      // Assume root node if no node is found
    //     }
    // }
    //
    // // Returns the current node, which is the last node in the open elements stack
    // fn get_current_node(&self) -> Option<&Node> {
    //     match self.open_elements.last() {
    //         Some(node_id) => self.document.get_node_by_id(*node_id),
    //         None => None,
    //     }
    // }

    // Pop all elements back to a table context
    fn clear_stack_back_to_table_context(&mut self) {
        while self.open_elements.len() > 0 {
            if ["tbody", "tfoot", "thead", "template", "html"].contains(&current_node!(self).name.as_str()) {
                return;
            }
            self.open_elements.pop();
        }
    }

    // Pop all elements back to a table row context
    fn clear_stack_back_to_table_row_context(&mut self) {
        while self.open_elements.len() > 0 {
            let val = current_node!(self).name.clone();
            if ["tr", "template", "html"].contains(&val.as_str()) {
                return;
            }
            self.open_elements.pop();
        }
    }

    // Checks if the given element is in given scope
    fn in_scope(&self, tag: &str, scope: Scope) -> bool {
        let mut idx = self.open_elements.len() - 1;
        loop {
            let node = open_elements_get!(self, idx);
            if node.name == tag {
                return true;
            }

            match scope {
                Scope::Regular => {
                    if ["applet", "caption", "html", "table", "td", "th", "marquee", "object"].contains(&node.name.as_str()) {
                        return false;
                    }
                }
                Scope::ListItem => {
                    if ["applet", "caption", "html", "table", "td", "th", "marquee", "object", "ol", "ul"].contains(&node.name.as_str()) {
                        return false;
                    }
                }
                Scope::Button => {
                    if ["applet", "caption", "html", "table", "td", "th", "marquee", "object", "button"].contains(&node.name.as_str()) {
                        return false;
                    }
                }
                Scope::Table => {
                    if ["html", "table", "template"].contains(&node.name.as_str()) {
                        return false;
                    }
                }
                Scope::Select => {
                    if ! ["optgroup", "option"].contains(&node.name.as_str()) {
                        return false;
                    }
                }
            }

            idx -= 1;
        }
    }

    fn close_cell(&mut self) {
        self.generate_all_implied_end_tags(None, false);

        let tag = current_node!(self).name.clone();
        if tag != "td" && tag != "th" {
            self.parse_error("current node should be td or th");
            return;
        }

        pop_until_any!(self, ["td", "th"]);

        self.clear_active_formatting_elements_until_marker();
        self.insertion_mode = InsertionMode::InRow;
    }


    fn handle_in_body(&mut self) {
        match &self.current_token {
            Token::TextToken { .. } if self.current_token.is_null() => {
                self.parse_error("null character not allowed in in body insertion mode");
                // ignore token
            },
            Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                self.reconstruct_formatting();

                let node = self.create_node(&self.current_token);
                self.document.add_node(node, current_node!(self).id);
            },
            Token::TextToken { .. } => {
                self.reconstruct_formatting();

                let node = self.create_node(&self.current_token);
                self.document.add_node(node, current_node!(self).id);

                self.frameset_ok = false;
            },
            Token::CommentToken { .. } => {
                let node = self.create_node(&self.current_token);
                self.document.add_node(node, current_node!(self).id);
            },
            Token::DocTypeToken { .. } => {
                self.parse_error("doctype not allowed in in body insertion mode");
                // ignore token
            },
            Token::StartTagToken { name, attributes, .. } if name == "html" => {
                self.parse_error("html tag not allowed in in body insertion mode");

                if open_elements_has!(self, "template") {
                    // ignore token
                    return;
                }

                if self.open_elements.is_empty() {
                    // ignore token
                    return;
                }

                if let NodeData::Element { attributes: node_attributes, .. } = &mut current_node_mut!(self).data {
                    for attr in attributes {
                        if !node_attributes.iter().any(|a| a.name == attr.name) {
                            node_attributes.push(Attribute{name: attr.name.clone(), value: attr.value.clone()});
                        }
                    }
                };
            },
            Token::StartTagToken { name, .. } if name == "base" || name == "basefont" || name == "bgsound" || name == "link" || name == "meta" || name == "noframes" || name == "script" || name == "style" || name == "template" || name == "title" => {
                self.handle_in_head();
            },
            Token::EndTagToken { name, .. } if name == "template" => {
                self.handle_in_head();
            },
            Token::StartTagToken { name, .. } if name == "body" => {
                self.parse_error("body tag not allowed in in body insertion mode");

                if self.open_elements.len() == 1 || open_elements_by_index!(self, 1).name != "body" {
                    // ignore token
                    return;
                }

                if self.frameset_ok == false {
                    // ignore token
                    return;
                }

                // Remove second element from parent node if has obe
                self.open_elements.remove(1);

                // pop all notes from bottom stack, from the current node up to the html element
                // insert html element for token
                // switch insertion mode to inframeset
                self.insertion_mode = InsertionMode::InFrameset;
            },
            _ => {}
        }
    }

    fn handle_in_head(&mut self) {
        let mut anything_else = false;

        match &self.current_token {
            Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                let node = self.create_node(&self.current_token);
                self.document.add_node(node, current_node!(self).id);
            },
            Token::CommentToken { .. } => {
                let node = self.create_node(&self.current_token);
                self.document.add_node(node, current_node!(self).id);
            },
            Token::DocTypeToken { .. } => {
                self.parse_error("doctype not allowed in before head insertion mode");
            },
            Token::StartTagToken { name, is_self_closing, .. } if name == "base" || name == "basefont" || name == "bgsound" || name == "link"  => {
                let node = self.create_node(&self.current_token);
                self.document.add_node(node, current_node!(self).id);

                self.open_elements.pop();

                if *is_self_closing {
                    let ct = &self.current_token.clone();
                    self.acknowledge_self_closing_tag(ct);
                }
            },
            Token::StartTagToken { name, is_self_closing, .. } if name == "meta" => {
                let node = self.create_node(&self.current_token);
                self.document.add_node(node, current_node!(self).id);

                self.open_elements.pop();

                if *is_self_closing {
                    self.acknowledge_self_closing_tag(&self.current_token.clone());
                }

                // @TODO: if active speculative html parser is null then...
            }
            Token::StartTagToken { name, .. } if name == "title" => {
                // @TODO: generic RCData parsing
            }
            Token::StartTagToken { name, .. } if name == "noscript" && self.scripting_enabled => {
                // @TODO: Generic Raw Text parsing
            },
            Token::StartTagToken { name, .. } if name == "noframes" || name == "style" => {
                // @TODO: generic RCData parsing
            }
            Token::StartTagToken { name, .. } if name == "noscript" && ! self.scripting_enabled => {
                let node = self.create_node(&self.current_token);
                let node_id = self.document.add_node(node, current_node!(self).id);
                self.open_elements.push(node_id);

                self.insertion_mode = InsertionMode::InHeadNoscript;
            }
            Token::StartTagToken { name, .. } if name == "script" => {
                // @TODO: generic RCData parsing
            }
            Token::EndTagToken { name } if name == "head" => {
                self.open_elements.pop();

                self.insertion_mode = InsertionMode::AfterHead;
            }
            Token::EndTagToken { name } if name == "body" || name == "html" || name == "br" => {
                anything_else = true;
            }
            Token::StartTagToken { name, .. } if name == "template" => {
                let node = self.create_node(&self.current_token);
                let node_id = self.document.add_node(node, current_node!(self).id);
                self.open_elements.push(node_id);

                self.add_marker();
                self.frameset_ok = false;

                self.insertion_mode = InsertionMode::InTemplate;
                self.template_insertion_mode.push(InsertionMode::InTemplate);

            }
            Token::EndTagToken { name, .. } if name == "template" => {
                if ! open_elements_has!(self, "template") {
                    self.parse_error("could not find template tag in open element stack");
                    return;
                }

                self.generate_all_implied_end_tags(None, true);

                if current_node!(self).name != "template" {
                    self.parse_error("template end tag not at top of stack");
                }

                pop_until!(self, "template");
                self.clear_active_formatting_elements_until_marker();
                self.template_insertion_mode.pop();

                self.reset_insertion_mode();
            }
            Token::StartTagToken { name, .. } if name == "head" => {
                self.parse_error("head tag not allowed in in head insertion mode");
            }
            Token::EndTagToken { .. } => {
                self.parse_error("end tag not allowed in in head insertion mode");
            },
            _ => {
                anything_else = true;
            }
        }
        if anything_else {
            self.open_elements.pop();
            self.insertion_mode = InsertionMode::AfterHead;
            self.reprocess_token = true;
        }
    }

    fn handle_in_template(&mut self) {
    }

    fn handle_in_table(&mut self) {
        let mut anything_else = false;

        match &self.current_token {
            Token::TextToken { .. } if ["table", "tbody", "template", "tfoot", "tr"].iter().any(|&node| node ==current_node!(self).name) => {
                self.pending_table_character_tokens = Vec::new();
                self.original_insertion_mode = self.insertion_mode;
                self.insertion_mode = InsertionMode::InTableText;
                self.reprocess_token = true;
            }
            Token::CommentToken { .. } => {
                let node = self.create_node(&self.current_token);
                let node_id = self.document.add_node(node, current_node!(self).id);
                self.open_elements.push(node_id);
            }
            Token::DocTypeToken { .. } => {
                self.parse_error("doctype not allowed in in table insertion mode");
            }
            Token::StartTagToken { name, .. } if name == "caption" => {
                self.clear_stack_back_to_table_context();

                self.add_marker();

                let node = self.create_node(&self.current_token);
                let node_id = self.document.add_node(node, current_node!(self).id);
                self.open_elements.push(node_id);

                self.insertion_mode = InsertionMode::InCaption;
            }
            Token::StartTagToken { name, .. } if name == "colgroup" => {
                self.clear_stack_back_to_table_context();

                let node = self.create_node(&self.current_token);
                let node_id = self.document.add_node(node, current_node!(self).id);
                self.open_elements.push(node_id);

                self.insertion_mode = InsertionMode::InColumnGroup;
            }
            Token::StartTagToken { name, .. } if name == "col" => {
                self.clear_stack_back_to_table_context();

                let token = Token::StartTagToken { name: "colgroup".to_string(), is_self_closing: false, attributes: Vec::new() };
                let node = self.create_node(&token);
                let node_id = self.document.add_node(node, current_node!(self).id);
                self.open_elements.push(node_id);

                self.insertion_mode = InsertionMode::InColumnGroup;
                self.reprocess_token = true;
            }
            Token::StartTagToken { name, .. } if name == "tbody" || name == "tfoot" || name == "thead" => {
                self.clear_stack_back_to_table_context();

                let node = self.create_node(&self.current_token);
                let node_id = self.document.add_node(node, current_node!(self).id);
                self.open_elements.push(node_id);

                self.insertion_mode = InsertionMode::InTableBody;
            }
            Token::StartTagToken { name, .. } if name == "td" || name == "th" || name == "tr" => {
                self.clear_stack_back_to_table_context();

                let token = Token::StartTagToken { name: "tbody".to_string(), is_self_closing: false, attributes: Vec::new() };
                let node = self.create_node(&token);
                let node_id = self.document.add_node(node, current_node!(self).id);
                self.open_elements.push(node_id);

                self.insertion_mode = InsertionMode::InTableBody;
                self.reprocess_token = true;
            }
            Token::StartTagToken { name, .. } if name == "table" => {
                self.parse_error("table tag not allowed in in table insertion mode");

                if ! open_elements_has!(self, "table") {
                    // ignore token
                    return;
                }

                pop_until!(self, "table");
                self.reset_insertion_mode();
                self.reprocess_token = true;
            }
            Token::EndTagToken { name, .. } if name == "table" => {
                if ! open_elements_has!(self, "table") {
                    self.parse_error("table end tag not allowed in in table insertion mode");
                    return;
                }

                pop_until!(self, "table");
                self.reset_insertion_mode();
            }
            Token::EndTagToken { name, .. } if name == "body" || name == "caption" || name == "col" || name == "colgroup" || name == "html" || name == "tbody" || name == "td" || name == "tfoot" || name == "th" || name == "thead" || name == "tr" => {
                self.parse_error("end tag not allowed in in table insertion mode");
                return;
            }
            Token::StartTagToken { name, .. } if name == "style" || name == "script" || name == "template" => {
                self.handle_in_head();
            }
            Token::EndTagToken { name, .. } if name == "template" => {
                self.handle_in_head();
            }
            Token::StartTagToken { name, is_self_closing, attributes } if name == "input" => {
                if !attributes.iter().any(|a| a.name == "type" && a.name == "hidden") {
                    anything_else = true;
                } else {
                    self.parse_error("input tag not allowed in in table insertion mode");

                    let node = self.create_node(&self.current_token);
                    self.document.add_node(node, current_node!(self).id);

                    pop_check!(self, "input");

                    if *is_self_closing {
                        self.acknowledge_self_closing_tag(&self.current_token.clone());
                    }
                }
            }
            Token::StartTagToken { name, attributes, .. } if name == "form" => {
                self.parse_error("form tag not allowed in in table insertion mode");

                if !attributes.iter().any(|a| a.name == "template") || self.form_element.is_none() {
                    // ignore token
                    return;
                }

                let node = self.create_node(&self.current_token);
                let node_id = self.document.add_node(node, current_node!(self).id);
                self.form_element = Some(node_id);

                pop_check!(self, "form");
            }
            Token::EofToken => {
                self.handle_in_body();
            }
            _ => anything_else = true,
        }

        if anything_else {
            self.parse_error("anything else not allowed in in table insertion mode");

            self.foster_parenting = true;
            self.handle_in_body();
            self.foster_parenting = false;
        }
    }

    fn handle_in_select(&mut self) {
        todo!()
    }

    fn reconstruct_formatting(&mut self) {
        todo!()
    }

    fn stop_parsing(&self) {
        todo!()
    }
}