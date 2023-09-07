use crate::html5_parser::node::Node;
use crate::html5_parser::node_arena::NodeArena;
use crate::html5_parser::parser::quirks::QuirksMode;

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum DocumentType {
    HTML,
    IframeSrcDoc,
}

pub struct Document {
    arena: NodeArena,
    pub doctype: DocumentType,    // Document type
    pub quirks_mode: QuirksMode,  // Quirks mode
}

impl Document {
    // Creates a new document
    pub fn new() -> Self {
        Self {
            arena: NodeArena::new(),
            doctype: DocumentType::IframeSrcDoc,
            quirks_mode: QuirksMode::NoQuirks,
        }
    }

    // Fetches a node by id or returns None when no node with this ID is found
    pub fn get_node_by_id(&self, node_id: usize) -> Option<&Node> {
        self.arena.get_node(node_id)
    }

    // Add to the document
    pub fn add_node(&mut self, node: Node, parent_id: usize) -> usize {
        let node_id = self.arena.add_node(node);
        self.arena.attach_node(parent_id, node_id);
        node_id
    }

    // Reattach a node to another parent
    pub fn reattach(&mut self, node_id: usize, parent_id: usize) {
        self.arena.attach_node(parent_id, node_id);
    }

    // return the root node
    pub fn get_root(&mut self) -> &Node {
        match self.arena.get_node(0) {
            Some(node) => node,
            None => {
                &Node::new_document()
            }
        }
    }
}