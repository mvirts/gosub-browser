use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use crate::html5_parser::tokenizer::token::Attribute;

#[derive(Debug, PartialEq)]
pub enum NodeType {
    Document,
    Text,
    Comment,
    Element,
}

pub enum NodeData {
    Document,
    Text { value: String },
    Comment { value: String },
    Element { name: String, attributes: Vec<Attribute> },
}

pub struct Node {
    pub parent: RefCell<Option<Rc<Node>>>,      // parent of the node, if any
    pub children: RefCell<Vec<Rc<Node>>>,       // children of the node
    pub name: String,                           // name of the node, or empty when its not a tag
    pub data: RefCell<NodeData>,                // actual data of the node
}

impl Node {
    pub fn new_document() -> Self {
        Node {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: RefCell::new(NodeData::Document),
            name: "".to_string(),
        }
    }
    pub fn new_element(name: &str, attributes: Vec<Attribute>) -> Self {
        Node {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: RefCell::new(NodeData::Element {
                name: name.to_string(),
                attributes: attributes,
            }),
            name: name.to_string(),
        }
    }
    pub fn new_comment(value: &str) -> Self {
        Node {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: RefCell::new(NodeData::Comment {
                value: value.to_string(),
            }),
            name: "".to_string(),
        }
    }
    pub fn new_text(value: &str) -> Self {
        Node {
            parent: RefCell::new(None),
            children: RefCell::new(vec![]),
            data: RefCell::new(NodeData::Text {
                value: value.to_string(),
            }),
            name: "".to_string(),
        }
    }

    pub fn append_child(&mut self, child: Rc<Node>) {
        self.children.borrow_mut().push(child);
    }
    pub fn prepend_child(&mut self, child: Rc<Node>) {
        self.children.borrow_mut().insert(0, child.to_owned());
    }
    pub fn insert_child(&mut self, child: Rc<Node>, index: usize) {
        self.children.borrow_mut().insert(index, child.to_owned());
    }
}

pub trait NodeTrait {
    // Return the token type of the given token
    fn type_of(&self) -> NodeType;
}

// Each node implements the NodeTrait and has a type_of that will return the node type.
impl NodeTrait for Node {
    fn type_of(&self) -> NodeType {
        match *self.data.borrow() {
            NodeData::Document { .. } => NodeType::Document,
            NodeData::Text { .. } => NodeType::Text,
            NodeData::Comment { .. } => NodeType::Comment,
            NodeData::Element { .. } => NodeType::Element,
        }
    }
}

impl fmt::Display for Node {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name)?;

        for child in self.children.borrow().iter() {
            write!(f, "\n{}|- {}", "  ".repeat(2), child)?;
        }
        Ok(())
    }

    // fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    //     // Helper function to recursively format nodes with indentation
    //     fn format_node(node: &Node, f: &mut fmt::Formatter, indent: usize) -> fmt::Result {
    //         // Write the current node's value with the current indentation
    //         writeln!(f, "{:indent$}{}", "", node.value, indent = indent)?;
    //
    //         // If there are children, recursively format them with increased indentation
    //         for child in node.children.borrow().iter() {
    //             format_node(child, f, indent + 2)?; // Increase indentation by 2 spaces for children
    //         }
    //         Ok(())
    //     }
    //
    //     // Start formatting from the current node with 0 indentation
    //     format_node(self, f, 0)
    // }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_nodes() {
        let mut n = Node::new("foo");
        assert_eq!(n.value, "foo");
        assert_eq!(n.children.len(), 0);

        let n2 = Node::new("bar");
        let n3 = Node::new("baz");
        n.add_child(n2);
        n.add_child(n3);
        assert_eq!(n.children.len(), 2);

        assert_eq!(n.children[0].value, "bar");
        assert_eq!(n.children[0].children.len(), 0);

        assert_eq!(n.children[1].value, "baz");
        assert_eq!(n.children[1].children.len(), 0);
    }
}
