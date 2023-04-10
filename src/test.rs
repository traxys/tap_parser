use crate::{Error, TapParser};
use indoc::indoc;
use paste::paste;

#[allow(unused_macros)]
macro_rules! compile_warning {
    (
    $name:ident, $message:expr $(,)*
) => {
        mod $name {
            #[must_use = $message]
            struct CompileWarning;
            #[allow(dead_code, path_statements)]
            fn trigger_warning() {
                CompileWarning;
            }
        }
    };
}

#[cfg(not(feature = "serde"))]
compile_warning!(
    serde,
    "You should enable the `serde` feature to run all tests"
);

macro_rules! make_test {
    (SUCCESS: $name:ident, $document:expr $(,)?) => {
        #[cfg(feature = "serde")]
        #[test]
        fn $name() {
            let mut parser = TapParser::new();
            insta::assert_yaml_snapshot!(parser.parse($document).unwrap());
        }

        paste! {
            #[cfg(feature = "serde")]
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
                insta::assert_yaml_snapshot!(parser.parse(&nested_doc).unwrap());
            }
        }
    };
    (FAIL: $name:ident, $document:expr, $error:expr, $(,)?) => {
        #[cfg(feature = "serde")]
        #[test]
        fn $name() {
            let mut parser = TapParser::new();
            assert_eq!(parser.parse($document), Err($error));
            insta::assert_yaml_snapshot!(parser.statements());
        }

        paste! {
            #[cfg(feature = "serde")]
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
                insta::assert_yaml_snapshot!(parser.statements());
            }
        }
    };
}

make_test! {SUCCESS: empty,
    indoc! {"
            TAP version 14
            1..0
    "}
}

make_test! {SUCCESS: pragma,
    indoc! {"
            TAP version 14
            1..1
            pragma +strict
    "},
}

make_test! {FAIL: anything_line,
    indoc! {"
            TAP version 14
            1..1
            this is clearly not a valid line
    "},
    Error::UnknownLine("this is clearly not a valid line".into()),
}

make_test! {FAIL: duplicate_plan,
    indoc! {"
            TAP version 14
            1..1
            1..1
    "},
    Error::DuplicatedPlan,
}

make_test! {SUCCESS: escape_pound,
    indoc! {r#"
            TAP version 14
            1..1
            ok 1 - test with \# escaped \\ chars # SKIP
    "#},
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
}

make_test! {FAIL: subtest_eod,
    indoc! {"
            TAP version 14
            1..1
            # Subtest: subtest
            ok 1 - out of the subtest
    "},
    Error::UnexpectedEOD,
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
}

make_test! {SUCCESS: subtest_bare,
    indoc! {"
            TAP version 14
            1..1
                ok 1 - inside subtest
                1..1
            ok 1 - subtest
    "},
}

make_test! {SUCCESS: empty_with_reason,
    indoc! {"
            TAP version 14
            1..0 # no tests to run
    "},
}

make_test! {SUCCESS: comment,
    indoc! {"
            TAP version 14
            1..1
            #   This is a comment
    "},
}

make_test! {SUCCESS: single_sucess,
    indoc! {"
            TAP version 14
            1..1
            ok 1 - this is a success
    "},
}

#[cfg(feature = "serde")]
#[test]
fn trailing_lines() {
    let document = indoc! {"
            TAP version 14
            ok 1 - this is a success
            1..1
            These are trailing lines, and are such are ignored
    "};
    let mut parser = TapParser::new();

    insta::assert_yaml_snapshot!(parser.parse(document).unwrap())
}

#[cfg(feature = "serde")]
#[test]
fn trailing_empty() {
    let document = indoc! {"
            TAP version 14
            1..0
            These are trailing lines, and are such are ignored
    "};
    let mut parser = TapParser::new();

    insta::assert_yaml_snapshot!(parser.parse(document).unwrap())
}

#[cfg(feature = "serde")]
#[test]
fn trailing_lines_after_yaml() {
    let document = indoc! {"
            TAP version 14
            1..1
            ok 1 - this is a success
              ---
              ...
            These are trailing lines, and are such are ignored
    "};
    let mut parser = TapParser::new();

    insta::assert_yaml_snapshot!(parser.parse(document).unwrap())
}

make_test! {FAIL: empty_directive,
    indoc! {"
            TAP version 14
            1..1
            ok 1 - desc #
    "},
    Error::MalformedDirective("".into()),
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
}

make_test! {FAIL: bail,
    indoc! {"
            TAP version 14
            1..1
            Bail out! We wanted to
            ok 1 - desc
    "},
    Error::Bailed("We wanted to".into()),
}

make_test! {FAIL: yaml_after_yaml,
    indoc! {"
            TAP version 14
            1..2
            not ok 1 - failure
              ---
              failure:
                 - why not
              ...
              ---
    "},
    Error::InvalidYaml,
}

make_test! {FAIL: yaml_close_only,
    indoc! {"
            TAP version 14
            1..1
            not ok 1 - failure
              ...
    "},
    Error::InvalidYamlClose,
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
}

make_test! {SUCCESS: single_sucess_skip,
    indoc! {"
            TAP version 14
            1..1
            ok 1 - desc # SKIP
    "},
}

make_test! {SUCCESS: single_sucess_todo,
    indoc! {"
            TAP version 14
            1..1
            ok 1 - desc # TODO
    "},
}

make_test! {FAIL: malformed_directive,
    indoc! {"
            TAP version 14
            1..1
            ok 1 - desc # INVALID
    "},
    Error::MalformedDirective("INVALID".into()),
}

make_test! {FAIL: malformed_directive_too_short,
    indoc! {"
            TAP version 14
            1..1
            ok 1 - desc # SML
    "},
    Error::MalformedDirective("SML".into()),
}

make_test! {SUCCESS: single_sucess_skip_reason,
    indoc! {"
            TAP version 14
            1..1
            ok 1 - desc # SKIP  has no power
    "},
}

make_test! {SUCCESS: single_sucess_skip_mixed_case,
    indoc! {"
            TAP version 14
            1..1
            ok 1 - desc # sKiP
    "},
}

make_test! {SUCCESS: single_sucess_bare,
    indoc! {"
            TAP version 14
            1..1
            ok
    "},
}

make_test! {SUCCESS: single_sucess_num_only,
    indoc! {"
            TAP version 14
            1..1
            ok 1
    "},
}

make_test! {SUCCESS: single_sucess_num_bare_desc,
    indoc! {"
            TAP version 14
            1..1
            ok 1 this is a bare description - with a dash!
    "},
}

make_test! {SUCCESS: single_sucess_no_num_bare_desc,
    indoc! {"
            TAP version 14
            1..1
            ok this is a bare description - with a dash!
    "},
}

make_test! {SUCCESS: single_sucess_no_num_dash_desc,
    indoc! {"
            TAP version 14
            1..1
            ok - this is a dash description - with a dash!
    "},
}

make_test! {SUCCESS: plan_at_the_end,
    indoc! {"
            TAP version 14
            ok - this is a dash description - with a dash!
            1..1
    "},
}

make_test! {SUCCESS: sucess_fail_bare,
    indoc! {"
            TAP version 14
            1..1
            ok
            not ok
    "},
}

#[test]
fn no_version() {
    let document = indoc! {"
            1..0
    "};
    let mut parser = TapParser::new();
    assert_eq!(parser.parse(document), Err(crate::Error::NoVersion))
}

#[cfg(feature = "serde")]
#[test]
fn eod() {
    let mut parser = TapParser::new();
    assert_eq!(parser.parse("TAP version 14"), Err(Error::UnexpectedEOD));
    insta::assert_yaml_snapshot!(parser.statements());
}

#[cfg(feature = "serde")]
#[test]
fn empty_input() {
    let mut parser = TapParser::new();
    assert_eq!(parser.parse(""), Err(Error::NoVersion));
    insta::assert_yaml_snapshot!(parser.statements());
}

#[cfg(feature = "serde")]
#[test]
fn with_default() {
    let document = indoc! {"
            TAP version 14
            1..0
    "};
    let mut parser: TapParser = Default::default();
    insta::assert_yaml_snapshot!(parser.parse(document).unwrap())
}

make_test! {FAIL: unsupported_version,
    indoc! {"
            TAP version 42
    "},
    crate::Error::InvalidVersion("42".into()),
}
