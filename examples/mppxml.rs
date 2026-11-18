//! Developer tool: print what the .mpp reader makes of a file, as XML interchange text.
fn main() {
    let p = std::env::args().nth(1).expect("FILE.mpp");
    match plancraft::mpp::to_xml(std::path::Path::new(&p)) {
        Ok((xml, i)) => {
            println!("{xml}");
            eprintln!("{} tasks, {} resources, {} links, {} assignments", i.tasks, i.resources, i.links, i.assignments);
        }
        Err(e) => eprintln!("{e}"),
    }
}
