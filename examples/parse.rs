use tap_parser::TapParser;

fn main() {
    let file = std::env::args().nth(1).unwrap();
    let document = std::fs::read_to_string(file).unwrap();
    let mut parser = TapParser::new();
    let parsed = parser.parse(&document);

    println!("Parsed: {parsed:#?}")
}
