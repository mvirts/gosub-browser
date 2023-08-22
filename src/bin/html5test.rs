use std::{env, fs, io};
use std::process::exit;
use serde_json;
use serde_json::Value;
use gosub_engine::html5_parser::input_stream::InputStream;
use gosub_engine::html5_parser::token_states::State as TokenState;
use gosub_engine::html5_parser::tokenizer::{Options, Tokenizer};
use gosub_engine::html5_parser::token::{Token, TokenTrait, TokenType};

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

        println!("***");
        println!("*** Running {} tests from 🗄️ {:?}", container.tests.len(), path);
        println!("***");

        for test in container.tests {
            run_token_test(&test)
        }
        println!("");

        exit(1);
    }

    Ok(())
}

fn run_token_test(test: &Test)
{
    // if test.description != "End tag not closing RCDATA or RAWTEXT (ending with left-angle-bracket)" {
    //     return;
    // }

    println!("🧪 running test: {}", test.description);

    // run for each state
    let last_start_tag = test.last_start_tag.clone().unwrap().to_string();
    for state in test.initial_states.iter() {
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
        is.read_from_str(test.input.as_str(), None);
        let mut tknzr = Tokenizer::new(&mut is, Some(Options{
            initial_state: state,
            last_start_tag: last_start_tag.clone(),
        }));

        // There can be multiple tokens to match. Make sure we match all of them
        for expected_token in test.output.iter() {
            // println!("Trying to match output");
            let t = tknzr.next_token();
            // println!("Token: {}", t);
            if ! match_token(t, expected_token) {
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

fn match_token(have: Token, expected: &Vec<Value>) -> bool {
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
        Token::StartTagToken{..} => {
            println!("❌ Incorrect start tag (not implemented in testsuite)");
            return false;
        }
        Token::EndTagToken{name} => {
            if name.as_str() != expected.get(1).unwrap() {
                println!("❌ Incorrect end tag");
                return false;
            }
        }
        Token::CommentToken{value} => {
            if value.as_str() != expected.get(1).unwrap() {
                println!("❌ Incorrect text found in comment token");
                return false;
            }
        }
        Token::TextToken{value} => {
            if value.as_str() != expected.get(1).unwrap() {
                println!("❌ Incorrect text found in text token (want: {} got: {})", expected.get(1).unwrap(), value.as_str());
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