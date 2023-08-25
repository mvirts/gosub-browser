use std::{env, fs, io};
use std::process::exit;
use serde_json;
use serde_json::Value;
use gosub_engine::html5_parser::input_stream::InputStream;
use gosub_engine::html5_parser::token_states::{State as TokenState};
use gosub_engine::html5_parser::tokenizer::{Options, Tokenizer};
use gosub_engine::html5_parser::token::{Token, TokenTrait, TokenType};

extern crate regex;
use regex::Regex;

#[macro_use]
extern crate serde_derive;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub tests: Vec<Test>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Test {
    pub description: String,
    pub input: String,
    pub output: Vec<Vec<Value>>,
    #[serde(default)]
    pub errors: Vec<Error>,
    #[serde(default)]
    pub double_escaped: Option<bool>,
    #[serde(default)]
    pub initial_states: Vec<String>,
    pub last_start_tag: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub code: String,
    pub line: i64,
    pub col: i64,
}


fn main () -> io::Result<()> {
    let default_dir = "./html5lib-tests";
    let dir = env::args().nth(1).unwrap_or(default_dir.to_string());

    for entry in fs::read_dir(dir + "/tokenizer")? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() || path.extension().unwrap() != "test" {
            continue;
        }

        let contents = fs::read_to_string(&path)?;
        let container = serde_json::from_str(&contents);
        if container.is_err() {
            continue;
        }
        let container: Root = container.unwrap();

        println!("*** Running {} tests from 🗄️ {:?}", container.tests.len(), path);

        for test in container.tests {
            run_token_test(&test)
        }
    }

    Ok(())
}

fn run_token_test(test: &Test)
{
    // if test.description != "Non BMP-charref in attribute" {
    //     return;
    // }

    println!("🧪 running test: {}", test.description);


    // If no initial state is given, assume Data state
    let mut states = test.initial_states.clone();
    if states.len() == 0 {
        states.push(String::from("Data state"));
    }

    for state in states.iter() {
        let state= match state.as_str() {
            "PLAINTEXT state" => TokenState::PlaintextState,
            "RAWTEXT state" => TokenState::RawTextState,
            "RCDATA state" => TokenState::RcDataState,
            "Script data state" => TokenState::ScriptDataState,
            "CDATA section state" => TokenState::CDataSectionState,
            "Data state" => TokenState::DataState,
            _ => panic!("unknown state found in test: {} ", state)
        };

        let mut is = InputStream::new();


        let input = if test.double_escaped.unwrap_or(false) {
            escape(test.input.as_str())
        } else {
            test.input.to_string()
        };

        is.read_from_str(input.as_str(), None);
        let mut tknzr = Tokenizer::new(&mut is, Some(Options{
            initial_state: state,
            last_start_tag: test.last_start_tag.clone().unwrap_or(String::from("")),
        }));

        // There can be multiple tokens to match. Make sure we match all of them
        for expected_token in test.output.iter() {
            let t = tknzr.next_token();
            if ! match_token(t, expected_token, test.double_escaped.unwrap_or(false)) {
                exit(1);
            }

            if test.errors.len() > 0 {
                if ! match_errors(&tknzr, &test.errors) {
                    exit(1);
                }
            }
        }
    }

    println!("----------------------------------------");
}

fn match_errors(tknzr: &Tokenizer, errors: &Vec<Error>) -> bool {
    for want_err in errors {
        let mut found = false;

        for got_err in tknzr.get_errors() {
            if got_err.message == want_err.code && got_err.line as i64 == want_err.line && got_err.col as i64 == want_err.col {
                found = true;
                println!("✅ found parse error '{}' at {}:{}", got_err.message, got_err.line, got_err.col);
                break;
            }
        }
        if ! found {
            println!("❌ expected parse error '{}' at {}:{}", want_err.code, want_err.line, want_err.col);
            for got_err in tknzr.get_errors() {
                println!("    '{}' at {}:{}", got_err.message, got_err.line, got_err.col);
            }
            return false;
        }
    }

    return true;
}

fn match_token(have: Token, expected: &Vec<Value>, double_escaped: bool) -> bool {
    let tp = expected.get(0).unwrap();

    let expected_token_type = match tp.as_str().unwrap() {
        "DOCTYPE" => TokenType::DocTypeToken,
        "StartTag" => TokenType::StartTagToken,
        "EndTag" => TokenType::EndTagToken,
        "Comment" => TokenType::CommentToken,
        "Character" => TokenType::TextToken,
        _ => panic!("unknown output token type {:?}", tp.as_str().unwrap())
    };

    if have.type_of() != expected_token_type {
        println!("❌ Incorrect token type found (want: {:?}, got {:?})", expected_token_type, have.type_of());
        return false;
    }

    match have {
        Token::DocTypeToken{..} => {
            println!("❌ Incorrect doctype (not implemented in testsuite)");
            return false;
        }
        Token::StartTagToken{name, attributes, ..} => {
            let output = expected.get(1).unwrap().as_str().unwrap();
            // check name
            if name.ne(&output) {
                println!("❌ Incorrect start tag (wanted: '{}', got '{}'", name, output);
                return false;
            }

            if attributes.len() == 0 {
                println!("ok");
            }

            // check self-closing
            // if is_self_closing != expected.get(2).unwrap().as_bool().unwrap() {
            //     println!("❌ Incorrect start tag (self-closing is not {}", if is_self_closing { "true" } else { "false"});
            //     return false;
            // }

            // check attrs


        }
        Token::EndTagToken{name} => {
            let output_ref = expected.get(1).unwrap().as_str().unwrap();
            let output = if double_escaped { escape(output_ref) } else { output_ref.to_string() };

            if name.as_str() != output {
                println!("❌ Incorrect end tag");
                return false;
            }
        }
        Token::CommentToken{value} => {
            let output_ref = expected.get(1).unwrap().as_str().unwrap();
            let output = if double_escaped { escape(output_ref) } else { output_ref.to_string() };

            if value.as_str() != output {
                println!("❌ Incorrect text found in comment token");
                println!("    wanted: '{}', got: '{}'", output, value.as_str());
                return false;
            }
        }
        Token::TextToken{value} => {
            let output_ref = expected.get(1).unwrap().as_str().unwrap();
            let output = if double_escaped { escape(output_ref) } else { output_ref.to_string() };

            if value.ne(&output) {
                println!("❌ Incorrect text found in text token");
                println!("    wanted: '{}', got: '{}'", output, value.as_str());
                return false;
            }
        },
        Token::EofToken => {
            println!("❌ EOF token");
            return false;
        }
    }

    println!("✅ Test passed");
    return true;
}

fn escape(input: &str) -> String {
    let re = Regex::new(r"\\u([0-9a-fA-F]{4})").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        let hex_val = u32::from_str_radix(&caps[1], 16).unwrap();
        char::from_u32(hex_val).unwrap().to_string()
    }).into_owned()
}