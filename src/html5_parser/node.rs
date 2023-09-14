use std::collections::HashMap;

pub const HTML_NAMESPACE:    &str = "http://www.w3.org/1999/xhtml";
pub const MATHML_NAMESPACE:  &str = "http://www.w3.org/1998/Math/MathML";
pub const SVG_NAMESPACE:     &str = "http://www.w3.org/2000/svg";
pub const XLINK_NAMESPACE:   &str = "http://www.w3.org/1999/xlink";
pub const XML_NAMESPACE:     &str = "http://www.w3.org/XML/1998/namespace";
pub const XMLNS_NAMESPACE:   &str = "http://www.w3.org/2000/xmlns/";


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
    pub namespace: Option<String>,  // namespace of the node
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
            namespace: None,
        }
    }

    pub fn new_element(name: &str, attributes: HashMap<String, String>, namespace: &str) -> Self {
        Node {
            id: 0,
            parent: None,
            children: vec![],
            data: NodeData::Element{
                name: name.to_string(),
                attributes: attributes,
            },
            name: name.to_string(),
            namespace: Some(namespace.into())
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
            namespace: None,
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
            namespace: None,
        }
    }


    pub fn is_special(&self) -> bool {
        if self.namespace == Some(HTML_NAMESPACE.into()) {
            if SPECIAL_HTML_ELEMENTS.contains(&self.name.as_str()) {
                return true;
            }
        }
        if self.namespace == Some(MATHML_NAMESPACE.into()) {
            if SPECIAL_MATHML_ELEMENTS.contains(&self.name.as_str()) {
                return true;
            }
        }
        if self.namespace == Some(SVG_NAMESPACE.into()) {
            if SPECIAL_SVG_ELEMENTS.contains(&self.name.as_str()) {
                return true;
            }
        }

        false
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

pub static SPECIAL_HTML_ELEMENTS: [&str; 81] = [
    "applet", "area", "article", "aside", "base", "basefont", "bgsound", "blockquote", "body",
    "br", "button", "caption", "center", "col", "colgroup", "dd", "details", "dir", "div", "dl",
    "dt", "embed", "fieldset", "figcaption", "figure", "footer", "form", "frame", "frameset",
    "h1", "h2", "h3", "h4", "h5", "h6", "head", "header", "hgroup", "hr", "html", "iframe",
    "img", "input", "keygen", "li", "link", "listing", "main", "marquee", "menu", "meta", "nav",
    "noembed", "noframes", "noscript", "object", "ol", "p", "param", "plaintext", "pre", "script",
    "section", "select", "source", "style", "summary", "table", "tbody", "td", "template",
    "textarea", "tfoot", "th", "thead", "title", "tr", "track", "ul", "wbr", "xmp",
];

pub static SPECIAL_MATHML_ELEMENTS: [&str; 6] = [
    "mi", "mo", "mn", "ms", "mtext", "annotation-xml",
];

pub static SPECIAL_SVG_ELEMENTS: [&str; 3] = [
    "foreignObject", "desc", "title",
];


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
