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

make_test! {SUCCESS: pragma,
    indoc! {"
            TAP version 14
            1..1
            pragma +strict
        "},
    vec![TapStatement::Plan(crate::TapPlan {
        count: 1,
        reason: None,
    })],
}

make_test! {FAIL: anything_line,
    indoc! {"
            TAP version 14
            1..1
            this is clearly not a valid line
        "},
    Error::UnknownLine("this is clearly not a valid line".into()),
    vec![TapStatement::Plan(crate::TapPlan {
        count: 1,
        reason: None,
    })],
}

make_test! {FAIL: duplicate_plan,
    indoc! {"
            TAP version 14
            1..1
            1..1
        "},
    Error::DuplicatedPlan,
    vec![TapStatement::Plan(crate::TapPlan {
        count: 1,
        reason: None,
    })],
}

make_test! {SUCCESS: escape_pound,
    indoc! {r#"
            TAP version 14
            1..1
            ok 1 - test with \# escaped \\ chars # SKIP
        "#},
    vec![
        TapStatement::Plan(crate::TapPlan {
            count: 1,
            reason: None,
        }),
        TapStatement::TestPoint(crate::TapTest {
            result: true,
            number: Some(1),
            desc: Some(r#"test with \# escaped \\ chars"#),
            directive: Some(crate::TapDirective {
                kind: crate::DirectiveKind::Skip,
                reason: None,
            }),
            yaml: Vec::new(),
        }),
    ]
}

make_test! {FAIL: subtest_bail,
    indoc! {"
            TAP version 14
            1..1
            # Subtest: subtest
                ok 1 - inside subtest
            Bail out! Doing a subtest
        "},
    Error::Bailed("Doing a subtest".into()),
    vec![
        TapStatement::Plan(crate::TapPlan {
            count: 1,
            reason: None,
        }),
    ],
}

make_test! {FAIL: subtest_eod,
    indoc! {"
            TAP version 14
            1..1
            # Subtest: subtest
            ok 1 - out of the subtest
        "},
    Error::UnexpectedEOD,
    vec![
        TapStatement::Plan(crate::TapPlan {
            count: 1,
            reason: None,
        }),
    ],
}

make_test! {FAIL: subtest_misindent,
    indoc! {"
            TAP version 14
            1..1
            # Subtest: subtest
               ok 1 - with three spaces
            ok 1 - subtest
        "},
    Error::Misindent { expected: 4, line: "   ok 1 - with three spaces".into() },
    vec![
        TapStatement::Plan(crate::TapPlan {
            count: 1,
            reason: None,
        }),
    ],
}

make_test! {SUCCESS: subtest_yaml,
    indoc! {"
            TAP version 14
            1..1
            # Subtest: subtest
                ok 1 - inside subtest
                1..1
            ok 1 - subtest
              ---
              yaml_in_subtest
              ...
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
                yaml: vec!["yaml_in_subtest"],
            },
        }),
    ],
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

make_test! {SUCCESS: single_sucess_todo,
    indoc! {"
            TAP version 14
            1..1
            ok 1 - desc # TODO
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
                kind: crate::DirectiveKind::Todo,
                reason: None,
            }),
            yaml: Vec::new(),
        }),
    ],
}

make_test! {FAIL: malformed_directive,
    indoc! {"
            TAP version 14
            1..1
            ok 1 - desc # INVALID
        "},
    Error::MalformedDirective("INVALID".into()),
    vec![
        TapStatement::Plan(crate::TapPlan {
            count: 1,
            reason: None,
        }),
    ],
}

make_test! {FAIL: malformed_directive_too_short,
    indoc! {"
            TAP version 14
            1..1
            ok 1 - desc # SML
        "},
    Error::MalformedDirective("SML".into()),
    vec![
        TapStatement::Plan(crate::TapPlan {
            count: 1,
            reason: None,
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

make_test! {SUCCESS: plan_at_the_end,
    indoc! {"
            TAP version 14
            ok - this is a dash description - with a dash!
            1..1
        "},
    vec![
        TapStatement::TestPoint(crate::TapTest {
            result: true,
            number: None,
            desc: Some("this is a dash description - with a dash!"),
            directive: None,
            yaml: Vec::new(),
        }),
        TapStatement::Plan(crate::TapPlan {
            count: 1,
            reason: None,
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

#[test]
fn eod() {
    let mut parser = TapParser::new();
    assert_eq!(parser.parse("TAP version 14"), Err(Error::UnexpectedEOD));
    assert_statements(parser.statements(), vec![]);
}

#[test]
fn empty_input() {
    let mut parser = TapParser::new();
    assert_eq!(parser.parse(""), Err(Error::NoVersion));
    assert_statements(parser.statements(), vec![]);
}

#[test]
fn with_default() {
    let document = indoc! {"
            TAP version 14
            1..0
        "};
    let mut parser: TapParser = Default::default();
    assert_statements(
        parser.parse(document).unwrap(),
        vec![TapStatement::Plan(crate::TapPlan {
            count: 0,
            reason: None,
        })],
    )
}

make_test! {FAIL: unsupported_version,
    indoc! {"
            TAP version 42
        "},
    crate::Error::InvalidVersion("42".into()),
    vec![],
}
