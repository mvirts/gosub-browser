use std::{env, fs, io};

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

pub struct TestResults{
    tests: usize,               // Number of tests (as defined in the suite)
    assertions: usize,          // Number of assertions (different combinations of input/output per test)
    succeeded: usize,           // How many succeeded assertions
    failed: usize,              // How many failed assertions
    failed_position: usize,     // How many failed assertions where position is not correct
}

fn main () -> io::Result<()> {
    let default_dir = "./html5lib-tests";
    let dir = env::args().nth(1).unwrap_or(default_dir.to_string());

    let mut results = TestResults{
        tests: 0,
        assertions: 0,
        succeeded: 0,
        failed: 0,
        failed_position: 0,
    };
    
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

        println!("🏃‍♂️ Running {} tests from 🗄️ {:?}", container.tests.len(), path);

        for test in container.tests {
            run_token_test(&test, &mut results)
        }
    }

    println!("🏁 Tests completed: Ran {} tests, {} assertions, {} succeeded, {} failed ({} position failures)", results.tests, results.assertions, results.succeeded, results.failed, results.failed_position);
    Ok(())
}

fn run_token_test(test: &Test, results: &mut TestResults)
{
    if ! test.description.eq("</ \\u0000") {
        return;
    }

    println!("🧪 running test: {}", test.description);

    results.tests += 1;

    // If no initial state is given, assume Data state
    let mut states = test.initial_states.clone();
    if states.is_empty() {
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
        let mut tokenizer = Tokenizer::new(&mut is, Some(Options{
            initial_state: state,
            last_start_tag: test.last_start_tag.clone().unwrap_or(String::from("")),
        }));

        // There can be multiple tokens to match. Make sure we match all of them
        for expected_token in test.output.iter() {
            let t = tokenizer.next_token();
            if ! match_token(t, expected_token, test.double_escaped.unwrap_or(false)) {
                results.assertions += 1;
                results.failed += 1;
            }

            // Check error messages
            match match_errors(&tokenizer, &test.errors) {
                ErrorResult::Failure => {
                    results.assertions += 1;
                    results.failed += 1;
                },
                ErrorResult::PositionFailure => {
                    results.assertions += 1;
                    results.failed += 1;
                    results.failed_position += 1;
                },
                ErrorResult::Success => {
                    results.assertions += 1;
                    results.succeeded += 1;
                }
            }
        }
    }

    println!("----------------------------------------");
}

#[derive(PartialEq)]
enum ErrorResult {
    Success,
    Failure,
    PositionFailure,
}

fn match_errors(tokenizer: &Tokenizer, errors: &Vec<Error>) -> ErrorResult {
    let mut result = ErrorResult::Success;
    for want_err in errors {

        

        for got_err in tokenizer.get_errors() {
            if got_err.message != want_err.code {
                println!("❌ Expected parse error '{}' at {}:{}", want_err.code, want_err.line, want_err.col);
                result = ErrorResult::Failure;
            } else if got_err.line != want_err.line || got_err.col != want_err.col {
                println!("❌ Expected position error '{}' at {}:{}", want_err.code, want_err.line, want_err.col);
                result = ErrorResult::PositionFailure;
            }

            if result != ErrorResult::Success {
                println!("   Parser errors generated:");
                for got_err in tokenizer.get_errors() {
                    println!("     * '{}' at {}:{}", got_err.message, got_err.line, got_err.col);
                }

                return result;
            }

            println!("✅ Found parse error '{}' at {}:{}", got_err.message, got_err.line, got_err.col);
        }
    }

    result
}

fn match_token(have: Token, expected: &[Value], double_escaped: bool) -> bool {
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
        Token::DocTypeToken{name, force_quirks, pub_identifier, sys_identifier} => {
            let expected_name = expected.get(1).unwrap().as_str();
            let expected_pub = expected.get(2).unwrap().as_str();
            let expected_sys = expected.get(3).unwrap().as_str();
            let expected_quirk = expected.get(4).unwrap().as_bool();

            if expected_name.is_none() && ! name.is_none() {
                println!("❌ Incorrect doctype (no name expected, but got '{}')", name.unwrap());
                return false;
            }
            if expected_name.is_some() && expected_name != Some(name.clone().unwrap().as_str()) {
                println!("❌ Incorrect doctype (wanted name: '{}', got: '{}')", expected_name.unwrap(), name.unwrap().as_str());
                return false;
            }
            if expected_quirk.is_some() && expected_quirk.unwrap() == force_quirks {
                println!("❌ Incorrect doctype (wanted quirk: '{}')", expected_quirk.unwrap());
                return false;
            }
            if expected_pub != pub_identifier.as_deref() {
                println!("❌ Incorrect doctype (wanted pub id: '{:?}', got '{:?}')", expected_pub, pub_identifier);
                return false;
            }
            if expected_sys != sys_identifier.as_deref() {
                println!("❌ Incorrect doctype (wanted sys id: '{:?}', got '{:?}')", expected_sys, sys_identifier);
                return false;
            }

        }
        Token::StartTagToken{name, attributes, ..} => {
            let output = expected.get(1).unwrap().as_str().unwrap();
            // check name
            if name.ne(&output) {
                println!("❌ Incorrect start tag (wanted: '{}', got '{}'", name, output);
                return false;
            }

            // @TODO: check attributes!
            if attributes.is_empty() {
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
    true
}

fn escape(input: &str) -> String {
    let re = Regex::new(r"\\u([0-9a-fA-F]{4})").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        let hex_val = u32::from_str_radix(&caps[1], 16).unwrap();
        // special case for converting surrogate codepoints to char (pro-tip: you can't)
        if (0xD800..=0xDFFF).contains(&hex_val) {
            return caps[1].to_string();
        }
        char::from_u32(hex_val).unwrap().to_string()
    }).into_owned()
}