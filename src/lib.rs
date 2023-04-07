//! This crate is a parser for the [Test Anything Protocol](https://testanything.org).
//!
//! It handles all the TAP 14 features, including subtests. The main entrypoint is the [TapParser]
//! structure
//!
//! # Example
//!
//! ```rust
//! use tap_parser::{TapParser, TapStatement, TapPlan, TapTest};
//!
//! let document = "TAP version 14\n1..1\nok 1 - success\nnot ok 2 - fail";
//! let mut parser = TapParser::new();
//! assert_eq!(
//!     parser.parse(document).unwrap(),
//!     vec![
//!         TapStatement::Plan(TapPlan {
//!             count: 1,
//!             reason: None
//!         }),
//!         TapStatement::TestPoint(TapTest {
//!             result: true,
//!             number: Some(1),
//!             desc: Some("success"),
//!             directive: None,
//!             yaml: Vec::new(),
//!         }),
//!         TapStatement::TestPoint(TapTest {
//!             result: false,
//!             number: Some(2),
//!             desc: Some("fail"),
//!             directive: None,
//!             yaml: Vec::new(),
//!         }),
//!     ]
//! );
//!
//! ```

use std::num::ParseIntError;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(PartialEq, Eq, Debug)]
pub struct TapPlan<'a> {
    pub count: usize,
    pub reason: Option<&'a str>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(PartialEq, Eq, Debug)]
pub enum DirectiveKind {
    Skip,
    Todo,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(PartialEq, Eq, Debug)]
pub struct TapDirective<'a> {
    pub kind: DirectiveKind,
    pub reason: Option<&'a str>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(PartialEq, Eq, Debug)]
pub struct TapTest<'a> {
    pub result: bool,
    pub number: Option<usize>,
    pub desc: Option<&'a str>,
    pub directive: Option<TapDirective<'a>>,
    pub yaml: Vec<&'a str>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(PartialEq, Eq, Debug)]
pub struct TapSubDocument<'a> {
    name: Option<&'a str>,
    statements: Vec<TapStatement<'a>>,
    ending: TapTest<'a>,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(PartialEq, Eq, Debug)]
pub enum TapStatement<'a> {
    Plan(TapPlan<'a>),
    TestPoint(TapTest<'a>),
    Comment(&'a str),
    Subtest(TapSubDocument<'a>),
}

impl<'a> TapStatement<'a> {
    fn as_test_mut(&mut self) -> &mut TapTest<'a> {
        match self {
            Self::TestPoint(t) => t,
            Self::Subtest(t) => &mut t.ending,
            _ => panic!("Statement {self:?} was not a TestPoint/Subtest"),
        }
    }
}

enum State {
    Body,
    AfterTest,
    Yaml,
    Subtest,
}

pub struct TapParser<'a> {
    in_body: bool,
    done: bool,
    state: State,
    yaml_accumulator: Vec<&'a str>,
    statements: Vec<TapStatement<'a>>,
    read_plan: bool,
    sub_parser: Option<SubTapParser<'a>>,
}

struct SubTapParser<'a> {
    parser: Box<TapParser<'a>>,
    name: Option<&'a str>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("TAP file does not have a version")]
    NoVersion,
    #[error("Version `{0}` is invalid")]
    InvalidVersion(String),
    #[error("Unexpected end of document")]
    UnexpectedEOD,
    #[error("Could not read number")]
    InvalidNumber(#[from] ParseIntError),
    #[error("Directive `{0}` is invalid")]
    MalformedDirective(String),
    #[error("Indentation mismatch, expected {expected} spaces in `{line}`")]
    Misindent { expected: usize, line: String },
    #[error("Yaml must directly follow a test point")]
    InvalidYaml,
    #[error("A closing yaml line must be preceded by an opening line")]
    InvalidYamlClose,
    #[error("Bailed: `{0}`")]
    Bailed(String),
    #[error("Line is unknown: {0}")]
    UnknownLine(String),
}

///
/// Entrypoint of this library. This struct holds the document state while parsing.
/// You should only need to call [parse](Self::parse).
///
impl<'a> TapParser<'a> {
    pub fn new() -> Self {
        Self {
            in_body: false,
            done: false,
            yaml_accumulator: Vec::new(),
            statements: Vec::new(),
            read_plan: false,
            state: State::Body,
            sub_parser: None,
        }
    }

    fn read_test_line(&mut self, result: bool, test: &'a str) -> Result<TapTest<'a>, Error> {
        let (number, end): (Option<usize>, _) = match test.split_once(' ') {
            Some((n, end)) if n.chars().all(|c| c.is_ascii_digit()) => (Some(n.parse()?), end),
            None if !test.is_empty() && test.chars().all(|c| c.is_ascii_digit()) => {
                (Some(test.parse()?), "")
            }
            _ => (None, test),
        };

        let end = end.strip_prefix('-').unwrap_or(end).trim();

        let mut escaped = false;
        let directive_start = end.as_bytes().iter().enumerate().find(|(_, c)| match c {
            b'\\' => {
                escaped = !escaped;
                false
            }
            b'#' if !escaped => true,
            b'#' if escaped => {
                escaped = false;
                false
            }
            _ => false,
        });

        let mut desc = end;
        let mut directive = None;
        if let Some((idx, _)) = directive_start {
            if idx == end.len() - 1 {
                return Err(Error::MalformedDirective("".into()));
            }

            desc = end[..idx].trim();
            let directive_str = end[idx + 1..].trim();
            if directive_str.len() < 4 {
                return Err(Error::MalformedDirective(directive_str.into()));
            }

            let directive_kind = directive_str[..4].to_lowercase();
            let reason = if directive_str.len() == 4 {
                None
            } else {
                Some(directive_str[4..].trim())
            };

            let kind = match directive_kind.as_str() {
                "skip" => DirectiveKind::Skip,
                "todo" => DirectiveKind::Todo,
                _ => return Err(Error::MalformedDirective(directive_str.into())),
            };

            directive = Some(TapDirective { kind, reason });
        }

        Ok(TapTest {
            result,
            number,
            desc: if desc.is_empty() { None } else { Some(desc) },
            directive,
            yaml: Vec::new(),
        })
    }

    fn read_body_line(&mut self, line: &'a str) -> Result<(), Error> {
        if let Some(pr) = line.strip_prefix("1..") {
            let (count, reason) = match pr.split_once('#') {
                None => (pr.trim().parse()?, None),
                Some((num, reason)) => (num.trim().parse()?, Some(reason.trim())),
            };

            self.statements
                .push(TapStatement::Plan(TapPlan { count, reason }));

            if self.in_body {
                self.done = true;
                todo!()
            } else {
                self.in_body = true;
            }

            self.read_plan = true;

            return Ok(());
        }

        match self.state {
            State::AfterTest if line == "  ---" => {
                self.state = State::Yaml;
                Ok(())
            }
            State::Subtest => {
                if line.len() >= 9 && line[0..9].to_lowercase() == "bail out!" {
                    Err(Error::Bailed(line[9..].trim().to_string()))
                } else if line.starts_with("ok") || line.starts_with("not ok") {
                    let sub_parser = self.sub_parser.take().unwrap();

                    if !(sub_parser.parser.done || sub_parser.parser.read_plan) {
                        return Err(Error::UnexpectedEOD);
                    }

                    let (result, test) = if let Some(test) = line.strip_prefix("ok") {
                        (true, test.trim())
                    } else if let Some(test) = line.strip_prefix("not ok") {
                        (false, test.trim())
                    } else {
                        unreachable!()
                    };

                    let sub_doc = TapSubDocument {
                        statements: sub_parser.parser.statements,
                        name: sub_parser.name,
                        ending: self.read_test_line(result, test)?,
                    };

                    self.statements.push(TapStatement::Subtest(sub_doc));
                    self.state = State::AfterTest;

                    Ok(())
                } else if line.len() < 4 || &line[0..4] != "    " {
                    Err(Error::Misindent {
                        expected: 4,
                        line: line.to_string(),
                    })
                } else if let Some(v) = line.strip_prefix("    TAP version") {
                    if v.trim() == "14" {
                        Ok(())
                    } else {
                        Err(Error::InvalidVersion(v.trim().into()))
                    }
                } else {
                    self.sub_parser
                        .as_mut()
                        .unwrap()
                        .parser
                        .read_body_line(&line[4..])
                }
            }
            State::Body | State::AfterTest => {
                if line.starts_with("    ")
                    || (line.len() >= 9 && line[0..9].to_lowercase() == "# subtest")
                {
                    self.state = State::Subtest;
                    let name = if line.starts_with('#') {
                        line.split_once(':').map(|(_, n)| n.trim())
                    } else {
                        None
                    };
                    let mut sub_parser = SubTapParser {
                        parser: Box::new(TapParser::new()),
                        name,
                    };
                    if let Some(line) = line.strip_prefix("    ") {
                        sub_parser.parser.read_body_line(line)?;
                    }
                    self.sub_parser = Some(sub_parser);
                    Ok(())
                } else if let Some(test_point) = line.strip_prefix("ok") {
                    let test = self.read_test_line(true, test_point.trim())?;
                    self.state = State::AfterTest;
                    self.statements.push(TapStatement::TestPoint(test));
                    Ok(())
                } else if let Some(test_point) = line.strip_prefix("not ok") {
                    let test = self.read_test_line(false, test_point.trim())?;
                    self.state = State::AfterTest;
                    self.statements.push(TapStatement::TestPoint(test));
                    Ok(())
                } else if line == "  ---" {
                    Err(Error::InvalidYaml)
                } else if line == "  ..." {
                    Err(Error::InvalidYamlClose)
                } else if line.len() >= 9 && line[0..9].to_lowercase() == "bail out!" {
                    Err(Error::Bailed(line[9..].trim().to_string()))
                } else if let Some(comment) = line.strip_prefix('#') {
                    self.statements.push(TapStatement::Comment(comment.trim()));
                    Ok(())
                } else if line.trim().is_empty() || line.starts_with("pragma ") {
                    Ok(())
                } else {
                    Err(Error::UnknownLine(line.into()))
                }
            }
            State::Yaml => {
                if line == "  ..." {
                    self.statements.last_mut().unwrap().as_test_mut().yaml =
                        std::mem::take(&mut self.yaml_accumulator);
                    self.state = State::Body;
                    Ok(())
                } else if line.len() < 2 || &line[..2] != "  " {
                    Err(Error::Misindent {
                        expected: 2,
                        line: line.to_string(),
                    })
                } else {
                    self.yaml_accumulator.push(&line[2..]);
                    Ok(())
                }
            }
        }
    }

    ///
    /// This function allows you to extract the statements from a parser even if parsing failed.
    /// All the statements may not be completely parsed.
    ///
    pub fn statements(self) -> Vec<TapStatement<'a>> {
        self.statements
    }

    ///
    /// This function will reset the internal state of the TAP parser. It will parse a TAP
    /// document into statements.
    ///
    /// In case of errors you can access the previous statements with the
    /// [statements](Self::statements) method
    ///
    pub fn parse(&mut self, input: &'a str) -> Result<Vec<TapStatement<'a>>, Error> {
        let mut lines = input.lines();
        let Some(first_line) = lines.next() else {
            return Err(Error::NoVersion);
        };

        let Some(version) = first_line.strip_prefix("TAP version") else {
            return Err(Error::NoVersion);
        };

        if version.trim() != "14" {
            return Err(Error::InvalidVersion(version.trim().to_string()));
        }

        for line in lines {
            if self.done {
                break;
            }

            self.read_body_line(line)?;
        }

        if !(self.done || self.read_plan) {
            return Err(Error::UnexpectedEOD);
        }

        Ok(std::mem::take(&mut self.statements))
    }
}

impl<'a> Default for TapParser<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use crate::{Error, TapParser, TapStatement, TapTest};
    use indoc::indoc;
    use paste::paste;

    fn assert_statements(body: Vec<TapStatement>, expected: Vec<TapStatement>) {
        body.iter()
            .zip(&expected)
            .enumerate()
            .for_each(|(i, (b, e))| {
                if b != e {
                    panic!("Statement {i} differs, expected: {b:#?}\ngot: {e:#?}");
                }
            });

        if body.len() != expected.len() {
            panic!("Statement length differ: expected: {expected:#?}\ngot: {body:#?}")
        }
    }

    macro_rules! make_test {
        (SUCCESS: $name:ident, $document:expr, $expected:expr $(,)?) => {
            #[test]
            fn $name() {
                let mut parser = TapParser::new();
                assert_statements(parser.parse($document).unwrap(), $expected);
            }

            paste! {
                #[test]
                fn [< $name _as_subtest >]() {
                    let mut nested_doc = indoc! {"
                        TAP version 14
                        1..1
                        # Subtest: inner
                    "}.to_string();
                    // Skip the version line
                    for line in $document.lines().skip(1) {
                        nested_doc += "    ";
                        nested_doc += line;
                        nested_doc += "\n";
                    }
                    nested_doc += "ok 1 - inner\n";
                    let mut parser = TapParser::new();
                    assert_statements(parser.parse(&nested_doc).unwrap(), vec![
                        TapStatement::Plan(crate::TapPlan{count: 1, reason: None}),
                        TapStatement::Subtest(crate::TapSubDocument{
                            name: Some("inner"),
                            statements: $expected,
                            ending: TapTest {
                                desc: Some("inner"),
                                directive: None,
                                yaml: Vec::new(),
                                number: Some(1),
                                result: true,
                            },
                        })
                    ]);
                }
            }
        };
        (FAIL: $name:ident, $document:expr, $error:expr, $parsed:expr $(,)?) => {
            #[test]
            fn $name() {
                let mut parser = TapParser::new();
                assert_eq!(parser.parse($document), Err($error));
                assert_statements(parser.statements(), $parsed);
            }

            paste! {
                #[test]
                fn [< $name _as_subtest >]() {
                    let mut nested_doc = indoc! {"
                        TAP version 14
                        1..1
                        # Subtest: inner
                    "}.to_string();
                    // Skip the version line
                    for line in $document.lines() {
                        nested_doc += "    ";
                        nested_doc += line;
                        nested_doc += "\n";
                    }
                    nested_doc += "ok 1 - inner\n";
                    let mut parser = TapParser::new();
                    println!("Document: {nested_doc}");
                    assert_eq!(parser.parse(&nested_doc), Err($error));
                    assert_statements(parser.statements(), vec![
                        TapStatement::Plan(crate::TapPlan{count: 1, reason: None}),
                        // TODO: provide *some* output of subtests in cases of errors
                        //
                        // TapStatement::Subtest(crate::TapSubDocument{
                        //     name: Some("inner"),
                        //     statements: $parsed,
                        //     ending: TapTest {
                        //         desc: Some("inner"),
                        //         directive: None,
                        //         yaml: Vec::new(),
                        //         number: Some(1),
                        //         result: true,
                        //     },
                        // })
                    ]);
                }
            }
        };
    }

    make_test! {SUCCESS: empty,
        indoc! {"
            TAP version 14
            1..0
        "},
        vec![TapStatement::Plan(crate::TapPlan {
            count: 0,
            reason: None,
        })],
    }

    make_test! {SUCCESS: subtest_with_name,
        indoc! {"
            TAP version 14
            1..1
            # Subtest: subtest
                ok 1 - inside subtest
                1..1
            ok 1 - subtest
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::Subtest(crate::TapSubDocument {
                name: Some("subtest"),
                statements: vec![
                    TapStatement::TestPoint(TapTest {
                        result: true,
                        directive: None,
                        desc: Some("inside subtest"),
                        yaml: Vec::new(),
                        number: Some(1),
                    }),
                    TapStatement::Plan(crate::TapPlan {
                        count: 1,
                        reason: None,
                    }),
                ],
                ending: crate::TapTest {
                    result: true,
                    number: Some(1),
                    desc: Some("subtest"),
                    directive: None,
                    yaml: Vec::new(),
                },
            }),
        ],
    }

    make_test! {SUCCESS: subtest_header,
        indoc! {"
            TAP version 14
            1..1
            # Subtest
                ok 1 - inside subtest
                1..1
            ok 1 - subtest
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::Subtest(crate::TapSubDocument {
                name: None,
                statements: vec![
                    TapStatement::TestPoint(TapTest {
                        result: true,
                        directive: None,
                        desc: Some("inside subtest"),
                        yaml: Vec::new(),
                        number: Some(1),
                    }),
                    TapStatement::Plan(crate::TapPlan {
                        count: 1,
                        reason: None,
                    }),
                ],
                ending: crate::TapTest {
                    result: true,
                    number: Some(1),
                    desc: Some("subtest"),
                    directive: None,
                    yaml: Vec::new(),
                },
            }),
        ],
    }

    make_test! {SUCCESS: subtest_bare,
        indoc! {"
            TAP version 14
            1..1
                ok 1 - inside subtest
                1..1
            ok 1 - subtest
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::Subtest(crate::TapSubDocument {
                name: None,
                statements: vec![
                    TapStatement::TestPoint(TapTest {
                        result: true,
                        directive: None,
                        desc: Some("inside subtest"),
                        yaml: Vec::new(),
                        number: Some(1),
                    }),
                    TapStatement::Plan(crate::TapPlan {
                        count: 1,
                        reason: None,
                    }),
                ],
                ending: crate::TapTest {
                    result: true,
                    number: Some(1),
                    desc: Some("subtest"),
                    directive: None,
                    yaml: Vec::new(),
                },
            }),
        ],
    }

    make_test! {SUCCESS: empty_with_reason,
        indoc! {"
            TAP version 14
            1..0 # no tests to run
        "},
        vec![TapStatement::Plan(crate::TapPlan {
            count: 0,
            reason: Some("no tests to run"),
        })],
    }

    make_test! {SUCCESS: comment,
        indoc! {"
            TAP version 14
            1..1
            #   This is a comment
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::Comment("This is a comment"),
        ],
    }

    make_test! {SUCCESS: single_sucess,
        indoc! {"
            TAP version 14
            1..1
            ok 1 - this is a success
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: true,
                number: Some(1),
                desc: Some("this is a success"),
                directive: None,
                yaml: Vec::new(),
            }),
        ],
    }

    make_test! {FAIL: empty_directive,
        indoc! {"
            TAP version 14
            1..1
            ok 1 - desc #
        "},
        Error::MalformedDirective("".into()),
        vec![TapStatement::Plan(crate::TapPlan{count: 1, reason: None})],
    }

    make_test! {FAIL: misindented_yaml,
        indoc! {"
            TAP version 14
            1..1
            not ok 1 - failure
              ---
             failure:
                 - why not
              ...
        "},
        Error::Misindent {
            expected: 2,
            line: " failure:".into()
        },
        vec![
            TapStatement::Plan(crate::TapPlan{count: 1, reason: None}),
            TapStatement::TestPoint(crate::TapTest{
                result: false,
                desc: Some("failure"),
                number: Some(1),
                directive: None,
                yaml: Vec::new(),
            }),
        ],
    }

    make_test! {FAIL: bail,
        indoc! {"
            TAP version 14
            1..1
            ok 1 - desc
            Bail out! We wanted to
        "},
        Error::Bailed("We wanted to".into()),
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: true,
                number: Some(1),
                desc: Some("desc"),
                directive: None,
                yaml: Vec::new(),
            }),
        ],
    }

    make_test! {FAIL: yaml_after_yaml,
        indoc! {"
            TAP version 14
            1..1
            not ok 1 - failure
              ---
              failure:
                 - why not
              ...
              ---
        "},
        Error::InvalidYaml,
        vec![
            TapStatement::Plan(crate::TapPlan{count: 1, reason: None}),
            TapStatement::TestPoint(crate::TapTest{
                result: false,
                desc: Some("failure"),
                number: Some(1),
                directive: None,
                yaml: vec![
                    "failure:",
                    "   - why not",
                ],
            }),
        ],
    }

    make_test! {FAIL: yaml_close_only,
        indoc! {"
            TAP version 14
            1..1
            not ok 1 - failure
              ...
        "},
        Error::InvalidYamlClose,
        vec![
            TapStatement::Plan(crate::TapPlan{count: 1, reason: None}),
            TapStatement::TestPoint(crate::TapTest{
                result: false,
                desc: Some("failure"),
                number: Some(1),
                directive: None,
                yaml: Vec::new(),
            }),
        ],
    }

    make_test! {SUCCESS: single_failure_yaml,
        indoc! {"
            TAP version 14
            1..1
            not ok 1 - failure
              ---
              failure:
                 - why not
              ...
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: false,
                number: Some(1),
                desc: Some("failure"),
                directive: None,
                yaml: vec!["failure:", "   - why not"],
            }),
        ],
    }

    make_test! {SUCCESS: single_sucess_skip,
        indoc! {"
            TAP version 14
            1..1
            ok 1 - desc # SKIP
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: true,
                number: Some(1),
                desc: Some("desc"),
                directive: Some(crate::TapDirective {
                    kind: crate::DirectiveKind::Skip,
                    reason: None,
                }),
                yaml: Vec::new(),
            }),
        ],
    }

    make_test! {SUCCESS: single_sucess_skip_reason,
        indoc! {"
            TAP version 14
            1..1
            ok 1 - desc # SKIP  has no power
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: true,
                number: Some(1),
                desc: Some("desc"),
                directive: Some(crate::TapDirective {
                    kind: crate::DirectiveKind::Skip,
                    reason: Some("has no power"),
                }),
                yaml: Vec::new(),
            }),
        ],
    }

    make_test! {SUCCESS: single_sucess_skip_mixed_case,
        indoc! {"
            TAP version 14
            1..1
            ok 1 - desc # sKiP
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: true,
                number: Some(1),
                desc: Some("desc"),
                directive: Some(crate::TapDirective {
                    kind: crate::DirectiveKind::Skip,
                    reason: None,
                }),
                yaml: Vec::new(),
            }),
        ],
    }

    make_test! {SUCCESS: single_sucess_bare,
        indoc! {"
            TAP version 14
            1..1
            ok
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: true,
                number: None,
                desc: None,
                directive: None,
                yaml: Vec::new(),
            }),
        ],
    }

    make_test! {SUCCESS: single_sucess_num_only,
        indoc! {"
            TAP version 14
            1..1
            ok 1
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: true,
                number: Some(1),
                desc: None,
                directive: None,
                yaml: Vec::new(),
            }),
        ],
    }

    make_test! {SUCCESS: single_sucess_num_bare_desc,
        indoc! {"
            TAP version 14
            1..1
            ok 1 this is a bare description - with a dash!
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: true,
                number: Some(1),
                desc: Some("this is a bare description - with a dash!"),
                directive: None,
                yaml: Vec::new(),
            }),
        ],
    }

    make_test! {SUCCESS: single_sucess_no_num_bare_desc,
        indoc! {"
            TAP version 14
            1..1
            ok this is a bare description - with a dash!
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: true,
                number: None,
                desc: Some("this is a bare description - with a dash!"),
                directive: None,
                yaml: Vec::new(),
            }),
        ],
    }

    make_test! {SUCCESS: single_sucess_no_num_dash_desc,
        indoc! {"
            TAP version 14
            1..1
            ok - this is a dash description - with a dash!
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: true,
                number: None,
                desc: Some("this is a dash description - with a dash!"),
                directive: None,
                yaml: Vec::new(),
            }),
        ],
    }

    make_test! {SUCCESS: sucess_fail_bare,
        indoc! {"
            TAP version 14
            1..1
            ok
            not ok
        "},
        vec![
            TapStatement::Plan(crate::TapPlan {
                count: 1,
                reason: None,
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: true,
                number: None,
                desc: None,
                directive: None,
                yaml: Vec::new(),
            }),
            TapStatement::TestPoint(crate::TapTest {
                result: false,
                number: None,
                desc: None,
                directive: None,
                yaml: Vec::new(),
            }),
        ],
    }

    #[test]
    fn no_version() {
        let document = indoc! {"
            1..0
        "};
        let mut parser = TapParser::new();
        assert_eq!(parser.parse(document), Err(crate::Error::NoVersion))
    }

    make_test! {FAIL: unsupported_version,
        indoc! {"
            TAP version 42
        "},
        crate::Error::InvalidVersion("42".into()),
        vec![],
    }
}
