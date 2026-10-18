//! Dev helper: prints a saved page's normalized `<main>` text, to write anchors.
//! `cargo run -q -p grid-parser --example text -- <file.html>`
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: text <file.html>")?;
    let page = grid_parser::Page::parse("x", &std::fs::read_to_string(path)?)?;
    println!("{}", page.text);
    Ok(())
}
