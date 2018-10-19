#[macro_use]
extern crate lazy_static;

extern crate clap;
extern crate mdbook;
extern crate pulldown_cmark;
extern crate pulldown_cmark_to_cmark;
extern crate regex;
extern crate serde_json;

use mdbook::book::{Book, BookItem, Chapter};
use mdbook::errors::{Error, Result};
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::MDBook;
use pulldown_cmark::{Event, Parser, Tag};
use pulldown_cmark_to_cmark::fmt::cmark;
use regex::Regex;
use std::env::{args, args_os};
use std::ffi::OsString;
use std::process;

fn main() {
    if args_os().count() != 2 {
        eprintln!("USAGE: {} <book>", args().next().expect("executable"));
        return;
    }
    if let Err(e) = handle_preprocessing(args_os().skip(1).next().expect("one argument")) {
        eprintln!("{}", e);
        process::exit(1);
    }
}

fn handle_preprocessing(json: OsString) -> Result<()> {
    let mut book = MDBook::load(json)?;
    book.with_preprecessor(ScalaWrapper);
    book.build()
}

lazy_static! {
    static ref WRAPPER_BLOCK_START: Regex = Regex::new(r"object wrapper.*{").unwrap();
}
fn is_wrapper_start(content: &str) -> bool {
    WRAPPER_BLOCK_START.is_match(content)
}

lazy_static! {
    static ref WRAPPER_BLOCK_END: Regex = Regex::new(r"^}").unwrap();
}
fn is_wrapper_end(content: &str) -> bool {
    WRAPPER_BLOCK_END.is_match(content)
}

pub struct ScalaWrapper;

impl ScalaWrapper {
    pub fn new() -> ScalaWrapper {
        ScalaWrapper
    }

    fn remove_wrappers(&self, chapter: &mut Chapter) -> Result<String> {
        let mut buf = String::with_capacity(chapter.content.len());
        let mut in_scala_block = false;
        let mut in_wrapped_block = false;
        let mut first_line = false;

        let events = Parser::new(&chapter.content).filter_map(|event| match event {
            Event::Start(Tag::CodeBlock(ref lang)) => {
                if lang.as_ref() == "scala" {
                    in_scala_block = true;
                    first_line = true;
                }
                Some(event)
            }

            Event::Text(ref content) => {
                if in_scala_block {
                    if first_line {
                        first_line = false;
                        if is_wrapper_start(&content) {
                            in_wrapped_block = true;
                            None
                        } else {
                            in_wrapped_block = false;
                            Some(event)
                        }
                    } else {
                        if in_wrapped_block {
                            if is_wrapper_end(&content) {
                                None
                            } else {
                                Some(event)
                            }
                        } else {
                            Some(event)
                        }
                    }
                } else {
                    Some(event)
                }
            }

            Event::End(Tag::CodeBlock(_)) => {
                in_scala_block = false;
                in_wrapped_block = false;
                Some(event)
            }
            other => Some(other),
        });

        cmark(events, &mut buf, None).map(|_| buf).map_err(|err| {
            Error::from(format!(
                "Markdown serialization failed within {}: {}",
                self.name(),
                err
            ))
        })
    }
}

impl Preprocessor for ScalaWrapper {
    fn name(&self) -> &str {
        "scala-wrapper-preprocessor"
    }

    fn run(&self, _ctx: &PreprocessorContext, book: &mut Book) -> Result<()> {
        eprintln!("Running '{}' preprocessor", self.name());
        let mut result: Result<()> = Ok(());
        let mut error = false;

        book.for_each_mut(|item: &mut BookItem| {
            if error {
                return;
            } else {
                if let BookItem::Chapter(ref mut chapter) = *item {
                    eprintln!("{}: processing chapter '{}'", self.name(), chapter.name);
                    result = match self.remove_wrappers(chapter) {
                        Ok(content) => {
                            chapter.content = content;
                            Ok(())
                        }

                        Err(err) => {
                            error = true;
                            Err(err)
                        }
                    }
                }
            }
        });

        result
    }
}
