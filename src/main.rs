use traversal::traverse;

mod parser;
mod traversal;

fn main() {
    let res = traverse("/home/leastinformednerd/Documents/Opinions".into());
    match res {
        Ok(parse) => {
            println!("{parse:#?}")
        }
        Err(err) => println!("err: {err:#?}"),
    }
}
