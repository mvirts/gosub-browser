use std::rc::Rc;
use crate::html5_parser::input_stream::InputStream;
use crate::html5_parser::node::Node;
use crate::html5_parser::quirks::QuirksMode;
use crate::html5_parser::token::Token;
use crate::html5_parser::tokenizer::{CHAR_NUL, Tokenizer};

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


trait VecExtensions<T> {
    fn pop_until<F>(&mut self, f: F) where F: FnMut(&T) -> bool;
    fn pop_check<F>(&mut self, f: F) -> bool where F: FnMut(&T) -> bool;
}

impl VecExtensions<Rc<Node>> for Vec<Rc<Node>> {
    fn pop_until<F>(&mut self, mut f: F)
    where
        F: FnMut(&Rc<Node>) -> bool,
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
        F: FnMut(&Rc<Node>) -> bool,
    {
        match self.pop() {
            Some(popped_value) => f(&popped_value),
            None => false,
        }
    }
}

pub struct Html5Parser<'a> {
    tokenizer: Tokenizer<'a>,                       // tokenizer object
    insertion_mode: InsertionMode,                  // current insertion mode
    original_insertion_mode: InsertionMode,         // original insertion mode (used for text mode)
    template_insertion_mode: Vec<InsertionMode>,    // template insertion mode stack
    parser_cannot_change_mode: bool,                // ??
    current_token: Token,                           // Current token from the tokenizer
    reprocess_token: bool,                          // If true, the current token should be processed again
    open_elements: Vec<Rc<Node>>,                   // Stack of open elements
    head_element: Option<Rc<Node>>,                 // Current head element
    form_element: Option<Rc<Node>>,                 // Current form element
    scripting_enabled: bool,                        // If true, scripting is enabled
    frameset_ok: bool,                              // if true, we can insert a frameset
    current_node: Option<Rc<Node>>,                 // Current node
    foster_parenting: bool,                         // Foster parenting flag
    script_already_started: bool,                   // If true, the script engine has already started
    pending_table_character_tokens: Vec<char>,      // Pending table character tokens
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum DocumentType {
    HTML,
    IframeSrcDoc,
}

pub struct Document {
    pub root: Rc<Node>,                             // Root of the document
    pub doctype: DocumentType,                      // Document type
    pub quirks_mode: QuirksMode,                    // Quirks mode
}

impl<'a> Html5Parser<'a> {
    // Creates a new parser object with the given input stream
    pub fn new(stream: &'a mut InputStream) -> Self {
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
            current_node: None,
            foster_parenting: false,
            script_already_started: false,
            pending_table_character_tokens: vec![],
        }
    }

    // Parses the input stream into a Node tree
    pub fn parse(&mut self) -> Document {
        let mut document = Document {
            root: self.create_node(&Token::TextToken { value: "root".to_string() }),
            doctype: DocumentType::HTML,
            quirks_mode: QuirksMode::NoQuirks,
            // _phantom: std::marker::PhantomData,
        };

        loop {
            // If reprocess_token is true, we should process the same token again
            if !self.reprocess_token {
                self.current_token = self.tokenizer.next_token();
            }
            self.reprocess_token = false;
            if self.current_token.is_eof() {
                break;
            }

            match self.insertion_mode {
                InsertionMode::Initial => {
                    match &self.current_token {
                        Token::CommentToken { .. } => {
                            document.root.add_child(self.create_node(&self.current_token));
                        }
                        Token::DocTypeToken { name, pub_identifier, sys_identifier, force_quirks } => {
                            if name.is_some() && name.as_ref().unwrap() != "html" ||
                                pub_identifier.is_some() ||
                                (sys_identifier.is_some() && sys_identifier.as_ref().unwrap() != "about:legacy-compat")
                            {
                                self.parse_error("doctype not allowed in initial insertion mode");
                            }

                            document.root.add_child(self.create_node(&self.current_token));

                            if document.doctype != DocumentType::IframeSrcDoc && self.parser_cannot_change_mode {
                                document.quirks_mode = self.identify_quirks_mode(name, pub_identifier.clone(), sys_identifier.clone(), *force_quirks);
                            }

                            self.insertion_mode = InsertionMode::BeforeHtml;
                        },
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            // ignore token
                        },
                        _ => {
                            if document.doctype != DocumentType::IframeSrcDoc {
                                self.parse_error("not an iframe doc src");
                            }

                            if self.parser_cannot_change_mode {
                                document.quirks_mode = QuirksMode::Quirks;
                            }

                            self.insertion_mode = InsertionMode::BeforeHtml;
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
                            document.root.add_child(self.create_node(&self.current_token));
                        }
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            // ignore token
                        }
                        Token::StartTagToken { name, .. } if name == "html" => {
                            let node = self.create_node(&self.current_token);
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);

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
                        self.open_elements.push(Rc::clone(&node));
                        document.root.add_child(node);

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
                            document.root.add_child(self.create_node(&self.current_token));
                        },
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in before head insertion mode");
                        },
                        Token::StartTagToken { name, .. } if name == "html" => {
                            // @TODO: Body insert mode rules

                            let node = self.create_node(&self.current_token);
                            self.head_element = Some(Rc::clone(&node));
                            document.root.add_child(node);

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
                        self.head_element = Some(Rc::clone(&node));
                        document.root.add_child(node);

                        self.insertion_mode = InsertionMode::InHead;
                        self.reprocess_token = true;
                    }
                }
                InsertionMode::InHead => {
                    let mut anything_else = false;
                    match &self.current_token {
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            document.root.add_child(self.create_node(&self.current_token));
                        },
                        Token::CommentToken { .. } => {
                            document.root.add_child(self.create_node(&self.current_token));
                        },
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in before head insertion mode");
                        },
                        Token::StartTagToken { name, is_self_closing, .. } if name == "base" || name == "basefont" || name == "bgsound" || name == "link"  => {
                            document.root.add_child(self.create_node(&self.current_token));
                            self.open_elements.pop();

                            if *is_self_closing {
                                self.acknowledge_self_closing_tag(&self.current_token);
                            }
                        },
                        Token::StartTagToken { name, is_self_closing, .. } if name == "meta" => {
                            document.root.add_child(self.create_node(&self.current_token));
                            self.open_elements.pop();

                            if *is_self_closing {
                                self.acknowledge_self_closing_tag(&self.current_token);
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
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);

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
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);

                            self.add_marker();
                            self.frameset_ok = false;

                            self.insertion_mode = InsertionMode::InTemplate;
                            self.template_insertion_mode.push(InsertionMode::InTemplate);

                        }
                        Token::EndTagToken { name, .. } if name == "template" => {
                            if ! self.open_elements.any(|node| node.value == "template") {
                                self.parse_error("could not find template tag in open element stack");
                                continue;
                            }

                            self.generate_all_implied_end_tags();

                            if self.current_node.get_data() != "template" {
                                self.parse_error("template end tag not at top of stack");
                                continue;
                            }

                            self.open_elements.pop_until(|node| node.value == "template");
                            self.clear_active_formatting_elements();
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
                InsertionMode::InHeadNoscript => {
                    let mut anything_else = false;
                    match &self.current_token {
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in 'head no script' insertion mode");
                        },
                        Token::StartTagToken { name, .. } if name == "html" => {
                            // @TODO: body insertion mode
                        },
                        Token::EndTagToken { name, .. } if name == "noscript" => {
                            if ! self.open_elements.pop_check(|node| node.value == "noscript") {
                                panic!("noscript tag should be popped from open elements");
                            }
                            // @TODO: set current node to head element
                            self.insertion_mode = InsertionMode::InHead;
                        },
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            // @TODO: process in head insertion mode
                        },
                        Token::CommentToken { .. } => {
                            // @TODO: process in head insertion mode
                        },
                        Token::StartTagToken { name, .. } if name == "basefont" || name == "bgsound" || name == "link" || name == "meta" || name == "noframes" || name == "style" => {
                            // @TODO: process in head insertion mode
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

                        if ! self.open_elements.pop_check(|node| node == "noscript") {
                            panic!("noscript tag should be popped from open elements");
                        }
                        // @TOD: current node set to head element?
                        self.current_node = self.head_element;

                        self.insertion_mode = InsertionMode::InHead;
                        self.reprocess_token = true;
                    }
                }
                InsertionMode::AfterHead => {
                    let mut anything_else = false;
                    match &self.current_token {
                        Token::TextToken { .. } if self.current_token.is_empty_or_white() => {
                            let node = self.create_node(&self.current_token);
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);
                        },
                        Token::CommentToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);
                        },
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in after head insertion mode");
                        },
                        Token::StartTagToken { name, .. } if name == "html" => {
                            // @TODO: body insertion mode
                        },
                        Token::StartTagToken { name, .. } if name == "body" => {
                            let node = self.create_node(&self.current_token);
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);

                            self.frameset_ok = true;
                            self.insertion_mode = InsertionMode::InBody;
                        },
                        Token::StartTagToken { name, .. } if name == "frameset" => {
                            let node = self.create_node(&self.current_token);
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);

                            self.insertion_mode = InsertionMode::InFrameset;
                        },

                        Token::StartTagToken { name, .. } if ["base", "basefront", "bgsound", "link", "meta", "noframes", "script", "style", "template", "title"].contains(&name.as_str()) => {
                            self.parse_error("invalid start tag in after head insertion mode");

                            self.open_elements.push(Rc::clone(self.head_element));

                            // @TODO: process following inhead insertion mode

                            // remove the node pointed to by the head element pointer from the stack of open elements (might not be current node at this point)
                        }
                        Token::EndTagToken { name, .. } if name == "template" => {
                            // @TODO; process inhead_insertion mode
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
                        document.root.add_child(node);

                        self.insertion_mode = InsertionMode::InBody;
                        self.reprocess_token = true;
                    }
                }
                InsertionMode::InBody => {}
                InsertionMode::Text => {
                    match &self.current_token {
                        Token::TextToken { .. } => {
                            let node = self.create_node(&self.current_token);
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);
                        },
                        Token::EofToken => {
                            self.parse_error("eof not allowed in text insertion mode");

                            if self.current_node.value == "script" {
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
                InsertionMode::InTable => {
                    let mut anything_else = false;

                    match &self.current_token {
                        Token::TextToken { value, .. } if ["table", "tbody", "template", "tfoot", "tr"].contains(self.current_node.name) => {
                            self.pending_table_character_tokens = Vec::new();
                            self.original_insertion_mode = self.insertion_mode;
                            self.insertion_mode = InsertionMode::InTableText;
                            self.reprocess_token = true;
                        }
                        Token::CommentToken { value } => {
                            let node = self.create_node(&self.current_token);
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);
                        }
                        Token::DocTypeToken { .. } => {
                            self.parse_error("doctype not allowed in in table insertion mode");
                        }
                        Token::StartTagToken { name, .. } if name == "caption" => {
                            self.clear_stack_back_to_table_context();

                            // @TODO: insert marker at the end of list
                            let node = self.create_node(&self.current_token);
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);

                            self.insertion_mode = InsertionMode::InCaption;
                        }
                        Token::StartTagToken { name, .. } if name == "colgroup" => {
                            self.clear_stack_back_to_table_context();

                            let node = self.create_node(&self.current_token);
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);

                            self.insertion_mode = InsertionMode::InColumnGroup;
                        }
                        Token::StartTagToken { name, .. } if name == "col" => {
                            self.clear_stack_back_to_table_context();

                            let token = Token::StartTagToken { name: "colgroup".to_string(), is_self_closing: false, attributes: Vec::new() };
                            let node = self.create_node(&token);
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);

                            self.insertion_mode = InsertionMode::InColumnGroup;
                            self.reprocess_token = true;
                        }
                        Token::StartTagToken { name, .. } if name == "tbody" || name == "tfoot" || name == "thead" => {
                            self.clear_stack_back_to_table_context();

                            let node = self.create_node(&self.current_token);
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);

                            self.insertion_mode = InsertionMode::InTableBody;
                        }
                        Token::StartTagToken { name, .. } if name == "td" || name == "th" || name == "tr" => {
                            self.clear_stack_back_to_table_context();

                            let token = Token::StartTagToken { name: "tbody".to_string(), is_self_closing: false, attributes: Vec::new() };
                            let node = self.create_node(&token);
                            self.open_elements.push(Rc::clone(&node));
                            document.root.add_child(node);

                            self.insertion_mode = InsertionMode::InTableBody;
                            self.reprocess_token = true;
                        }
                        Token::StartTagToken { name, .. } if name == "table" => {
                            self.parse_error("table tag not allowed in in table insertion mode");

                            if !self.open_elements.contains("table".as_str()) {
                                // ignore token
                                continue;
                            }

                            self.open_elements.pop_until(|node| node == "table");
                            self.reset_insertion_mode();
                            self.reprocess_token = true;
                        }
                        Token::EndTagToken { name, .. } if name == "table" => {
                            if !self.open_elements.has("table") {
                                self.parse_error("table end tag not allowed in in table insertion mode");
                                continue;
                            }

                            self.open_elements.pop_until(|node| node == "table");
                            self.reset_insertion_mode();
                        }
                        Token::EndTagToken { name, .. } if name == "body" || name == "caption" || name == "col" || name == "colgroup" || name == "html" || name == "tbody" || name == "td" || name == "tfoot" || name == "th" || name == "thead" || name == "tr" => {
                            self.parse_error("end tag not allowed in in table insertion mode");
                            continue;
                        }
                        Token::StartTagToken { name, .. } if name == "style" || name == "script" || name == "template" => {
                            // @TODO: process in head insertion mode
                        }
                        Token::EndTagToken { name, .. } if name == "template" => {
                            // @TODO: process in head insertion mode
                        }
                        Token::StartTagToken { name, is_self_closing, attributes } if name == "input" => {
                            if !attributes.iter().any(|a| a.name == "type" && a.value == "hidden") {
                                anything_else = true;
                            } else {
                                self.parse_error("input tag not allowed in in table insertion mode");

                                let node = self.create_node(&self.current_token);
                                document.root.add_child(node);
                                if ! self.open_elements.pop_check(|node| node == "input") {
                                    panic!("input tag should be popped from open elements");
                                }

                                if is_self_closing {
                                    self.acknowledge_self_closing_tag(&self.current_token);
                                }
                            }
                        }
                        Token::StartTagToken { name, is_self_closing, attributes } if name == "form" => {
                            self.parse_error("form tag not allowed in in table insertion mode");

                            if !attributes.iter().any(|a| a.name == "template") || self.form_element.is_none() {
                                // ignore token
                                continue;
                            }

                            let node = self.create_node(&self.current_token);
                            document.root.add_child(node);

                            self.form_element = &node;
                            if ! self.open_elements.pop_check(|node| node.value == "form") {
                                panic!("form tag should be popped from open elements");
                            }
                        }
                        Token::EofToken => {
                            // @TODO: process like in-body insertion mode
                        }
                        _ => anything_else = true,
                    }

                    if anything_else {
                        self.parse_error("anything else not allowed in in table insertion mode");

                        self.foster_parenting = true;
                        // @TODO: process like in-body insertion mode
                        self.foster_parenting = false;
                    }
                }
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

    // Creates a parse error and halts the parser
    fn parse_error(&self, message: &str) {
        println!("Parse error ({}/{}): {}", self.tokenizer.get_position().line, self.tokenizer.get_position().col, message);
        panic!();
    }

    // Create a new node onto the area and returns an optional reference
    fn create_node(&self, token: &Token) -> Rc<Node> {
        let val: String;
        match token {
            Token::DocTypeToken { name, pub_identifier, sys_identifier, force_quirks} => {
                val = format!("doctype[{} {} {} {}]",
                    name.as_deref().unwrap_or(""),
                    pub_identifier.as_deref().unwrap_or(""),
                    sys_identifier.as_deref().unwrap_or(""),
                    force_quirks
                );
            }
            Token::StartTagToken { name, is_self_closing, ..} => {
                val = format!("start_tag[{}, selfclosing: {}]", name, is_self_closing);
            }
            Token::EndTagToken { name } => {
                val = format!("end_tag[{}]", name);
            }
            Token::CommentToken { value } => {
                val = format!("comment[{}]", value);
            }
            Token::TextToken { value } => {
                val = format!("text[{}]", value);
            }
            Token::EofToken => {
                val = String::from("eof");
            }
        }

        Rc::new(Node::new(val.as_str()))
    }
    fn acknowledge_self_closing_tag(&self, token: &Token) {
        todo!()
    }
    fn clear_stack_back_to_table_context(&mut self) {
        if ["table", "template", "html"].contains(self.current_node.unwrap().value) {
            return;
        }

        self.open_elements.pop_until(|node| node.value == "table");
    }
    fn flush_pending_table_character_tokens(&self) {
        todo!()
    }
    fn add_marker(&self) {
        todo!()
    }
    fn generate_all_implied_end_tags(&self) {
        todo!()
    }
    fn clear_active_formatting_elements(&self) {
        todo!()
    }
    fn reset_insertion_mode(&self) {
        todo!()
    }
}