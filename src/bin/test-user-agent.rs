use std::process::exit;

use gosub_engine::html5_parser::input_stream::Confidence;
use gosub_engine::html5_parser::input_stream::{Encoding, InputStream};
use gosub_engine::html5_parser::node::{Node, NodeData};
use gosub_engine::html5_parser::parser::document::Document;
use gosub_engine::html5_parser::parser::Html5Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::args()
        .nth(1)
        .or_else(|| {
            println!("Usage: gosub-browser <url>");
            exit(1);
        })
        .unwrap();

    // Fetch the html from the url
    let response = reqwest::blocking::get(&url)?;
    if !response.status().is_success() {
        println!("could not get url. Status code {}", response.status());
        exit(1);
    }
    let html = response.text()?;

    let mut stream = InputStream::new();
    stream.read_from_str(&html, Some(Encoding::UTF8));
    stream.set_confidence(Confidence::Certain);

    // If the encoding confidence is not Confidence::Certain, we should detect the encoding.
    if !stream.is_certain_encoding() {
        stream.detect_encoding()
    }

    let mut parser = Html5Parser::new(&mut stream);
    let (document, parse_error) = parser.parse();

    match get_node_by_path(document, vec!["html", "body"]) {
        None => {
            println!("[No Body Found]");
        }
        Some(node) => display_node(document, node),
    }

    for e in parse_error {
        println!("Parse Error: {}", e.message)
    }

    Ok(())
}

fn get_node<'a>(document: &'a Document, parent: &'a Node, name: &'a str) -> Option<&'a Node> {
    for id in &parent.children {
        match document.get_node_by_id(*id) {
            None => {}
            Some(node) => {
                if node.name.eq(name) {
                    return Some(node);
                }
            }
        }
    }
    None
}

fn get_node_by_path<'a>(document: &'a Document, path: Vec<&'a str>) -> Option<&'a Node> {
    let mut node = document.get_root();
    match document.get_node_by_id(node.children[0]) {
        None => {
            return None;
        }
        Some(child) => {
            node = child;
        }
    }
    for name in path {
        match get_node(document, node, name) {
            Some(new_node) => {
                node = new_node;
            }
            None => {
                return None;
            }
        }
    }
    Some(node)
}

fn display_node(document: &Document, node: &Node) {
    if let NodeData::Text { value } = &node.data {
        if !value.eq("\n") {
            println!("{}", value);
        }
    }
    for child_id in &node.children {
        if let Some(child) = document.get_node_by_id(*child_id) {
            display_node(document, child);
        }
    }
}
