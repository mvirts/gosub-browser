pub mod input_stream;

pub mod token_states;
pub mod tokenizer;
pub mod token;
pub mod parser;

mod consume_char_refs;
mod emitter;
mod node;
mod token_named_characters;
mod token_replacements;