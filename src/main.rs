mod parser;

fn main() {
    let s = std::fs::read_to_string(
        "/home/leastinformednerd/Documents/Opinions/Art/Music/Songs/viceheart.md",
    )
    .expect("debug");
    let res = parser::parse_file(&s);
    match res {
        Ok(parse) => println!("{parse:#?}"),
        Err(err) => println!("err: {err:#?}"),
    }
}
