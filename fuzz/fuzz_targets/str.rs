#![no_main]
use libfuzzer_sys::fuzz_target;

use tap_parser::TapParser;

fuzz_target!(|data: &str| {
    let mut parser = TapParser::new();
    let _ = parser.parse(data);
});
