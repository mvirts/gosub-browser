use std::{env, fs, io};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use gosub_engine::html5_parser::input_stream::InputStream;
use gosub_engine::html5_parser::parser::Html5Parser;
use gosub_engine::html5_parser::parser::document::Document;

pub struct TestResults{
    tests: usize,               // Number of tests (as defined in the suite)
    assertions: usize,          // Number of assertions (different combinations of input/output per test)
    succeeded: usize,           // How many succeeded assertions
    failed: usize,              // How many failed assertions
    failed_position: usize,     // How many failed assertions where position is not correct
}

struct Test {
    file_path: String,                  // Filename of the test
    line: usize,                        // Line number of the test
    data: String,                       // input stream
    errors: Vec<String>,                // errors
    document: Vec<String>,              // document tree
    document_fragment: Vec<String>,     // fragment
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
    
    for entry in fs::read_dir(dir + "/tree-construction")? {
        let entry = entry?;
        let path = entry.path();

        if ! path.ends_with("tests1.dat") {
            continue;
        }

        if !path.is_file() || path.extension().unwrap() != "dat" {
            continue;
        }

        let tests = read_tests(path.clone())?;
        println!("🏃‍♂️ Running {} tests from 🗄️ {:?}\n", tests.len(), path);

        for test in tests {
            run_tree_test(&test, &mut results)
        }
    }

    println!("🏁 Tests completed: Ran {} tests, {} assertions, {} succeeded, {} failed ({} position failures)", results.tests, results.assertions, results.succeeded, results.failed, results.failed_position);
    Ok(())
}

fn read_tests(file_path: PathBuf) -> io::Result<Vec<Test>> {
    let file = File::open(file_path.clone())?;
    let reader = BufReader::new(file);

    let mut tests = Vec::new();
    let mut current_test = Test{
        file_path: file_path.to_str().unwrap().clone().to_string(),
        line: 1,
        data: "".to_string(),
        errors: vec![],
        document: vec![],
        document_fragment: vec![],
    };
    let mut section: Option<&str> = None;

    let mut line_num: usize = 0;
    for line in reader.lines() {
        line_num += 1;

        let line = line?;

        if line.starts_with("#data") {
            if !current_test.data.is_empty() || !current_test.errors.is_empty() || !current_test.document.is_empty() {
                tests.push(current_test);
                current_test = Test{
                    file_path: file_path.to_str().unwrap().clone().to_string(),
                    line: line_num,
                    data: "".to_string(),
                    errors: vec![],
                    document: vec![],
                    document_fragment: vec![],
                };
            }
            section = Some("data");
        } else if line.starts_with('#') {
            section = match line.as_str() {
                "#errors" => Some("errors"),
                "#document" => Some("document"),
                _ => None,
            };
        } else if let Some(sec) = section {
            match sec {
                "data" => current_test.data.push_str(&line),
                "errors" => current_test.errors.push(line),
                "document" => current_test.document.push(line),
                "document_fragment" => current_test.document_fragment.push(line),
                _ => (),
            }
        }
    }

    // Push the last test if it has data
    if !current_test.data.is_empty() || !current_test.errors.is_empty() || !current_test.document.is_empty() {
        tests.push(current_test);
    }

    Ok(tests)
}

fn run_tree_test(test: &Test, results: &mut TestResults)
{
    println!("🧪 Running test: {}::{}", test.file_path, test.line);

    results.tests += 1;

    let mut is = InputStream::new();
    is.read_from_str(test.data.as_str(), None);

    let mut document = Document::new();
    let mut parser = Html5Parser::new(&mut is, &mut document);
    parser.parse();

    println!("Generated tree: \n\n {}", document.get_root());

    println!("----------------------------------------");
}