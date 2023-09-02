use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;


pub struct Node {
    pub value: String,
    pub children: RefCell<Vec<Rc<Node>>>,
}

impl Node {
    pub fn new(value: &str) -> Self {
        Node {
            value: value.to_string(),
            children: RefCell::new(vec![]),
        }
    }

    pub fn add_child(&self, child: Rc<Node>) {
        self.children.borrow_mut().push(child);
    }
}


impl fmt::Display for Node {

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.value)?;

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
