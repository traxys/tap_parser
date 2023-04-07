fn main() {
    let input = std::env::args().nth(1).unwrap();
    let input = std::fs::read_to_string(input).unwrap();

    let mut parser = tap_parser::TapParser::new();
    let document = parser.parse(&input).unwrap();

    println!("{}", serde_json::to_string_pretty(&document).unwrap());
}
