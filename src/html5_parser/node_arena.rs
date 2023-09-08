use std::collections::HashMap;
use crate::html5_parser::node::Node;

pub struct NodeArena {
    nodes: HashMap<usize, Node>,        // Current nodes
    next_id: usize,                     // next id to use
}

impl NodeArena {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn get_node(&self, node_id: usize) -> Option<&Node> {
        self.nodes.get(&node_id)
    }

    pub fn get_mut_node(&mut self, node_id: usize) -> Option<&mut Node> {
        self.nodes.get_mut(&node_id)
    }

    pub fn add_node(&mut self, mut node: Node) -> usize {
        let id = self.next_id;
        self.next_id += 1;

        node.id = id;
        self.nodes.insert(id, node);
        id
    }

    pub fn attach_node(&mut self, parent_id: usize, node_id: usize) {
        if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
            parent_node.children.push(node_id);
        }
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.parent = Some(parent_id);
        }
    }

    fn remove_node(&mut self, node_id: usize) {
        if let Some(node) = self.nodes.remove(&node_id) {
            if let Some(parent_id) = node.parent {
                if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
                    parent_node.children.retain(|&id| id != node_id);
                }
            }
        }
    }
}

