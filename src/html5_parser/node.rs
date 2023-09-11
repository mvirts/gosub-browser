use std::collections::HashMap;

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
    Element { name: String, attributes: HashMap<String, String> },
}

pub struct Node {
    pub id: usize,                  // ID of the node, 0 is always the root / document node
    pub parent: Option<usize>,      // parent of the node, if any
    pub children: Vec<usize>,       // children of the node
    pub name: String,               // name of the node, or empty when its not a tag
    pub data: NodeData,             // actual data of the node
}

impl Node {
    pub fn new_document() -> Self {
        Node {
            id: 0,
            parent: None,
            children: vec![],
            data: NodeData::Document{},
            name: "".to_string(),
        }
    }

    pub fn new_element(name: &str, attributes: HashMap<String, String>) -> Self {
        Node {
            id: 0,
            parent: None,
            children: vec![],
            data: NodeData::Element{
                name: name.to_string(),
                attributes: attributes,
            },
            name: name.to_string(),
        }
    }

    pub fn new_comment(value: &str) -> Self {
        Node {
            id: 0,
            parent: None,
            children: vec![],
            data: NodeData::Comment{
                value: value.to_string(),
            },
            name: "".to_string(),
        }
    }

    pub fn new_text(value: &str) -> Self {
        Node {
            id: 0,
            parent: None,
            children: vec![],
            data: NodeData::Text{
                value: value.to_string(),
            },
            name: "".to_string(),
        }
    }
}

pub trait NodeTrait {
    // Return the token type of the given token
    fn type_of(&self) -> NodeType;
}

// Each node implements the NodeTrait and has a type_of that will return the node type.
impl NodeTrait for Node {
    fn type_of(&self) -> NodeType {
        match self.data {
            NodeData::Document { .. } => NodeType::Document,
            NodeData::Text { .. } => NodeType::Text,
            NodeData::Comment { .. } => NodeType::Comment,
            NodeData::Element { .. } => NodeType::Element,
        }
    }
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
