use std::path::Path;
use skog::{*, ForestEdge::*};

// Recursively build forest for given path.
fn build_forest(path: &Path) -> Forest<String> {
    let mut f = Forest::new();
    if !path.is_dir() {
        // Ignore files (and anything that's not a directory).
        return f;
    }
    let file_name = path.file_name().unwrap().to_str().unwrap();
    if file_name.starts_with('.') {
        // Ignore "hidden" directories.
        return f;
    }
    let mut cur = f.end_mut();
    // Make current directory the "root" and move cursor to new node.
    cur.insert_and_move(file_name.to_string());
    // Set cursor to trailing to append children to it.
    cur.trailing_of();
    for entry in path.read_dir().unwrap() {
        let entry = entry.unwrap();
        let dir = build_forest(&entry.path());
        cur.splice(dir);
    }
    f
}

// Helper type to print leading indentation.
struct Tabs(usize);

impl std::fmt::Display for Tabs {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        for _ in 0..self.0 {
            write!(f, "\t")?;
        }
        Ok(())
    }
}

// Print directories in xml format.
fn print(f: &Forest<String>) {
    let mut cur = f.begin();
    let mut depth = 0;
    while cur != f.end() {
        match (cur.edge(), cur.current().unwrap()) {
            // Entering directory, print opening tag and increase depth.
            (Leading, name) => {
                println!("{}<{}>", Tabs(depth), name);
                depth += 1;
            }
            // Exiting directory, decrease depth and print closing tag.
            (Trailing, name) => {
                depth -= 1;
                println!("{}</{}>", Tabs(depth), name);
            }
        }
        cur.move_next();
    }
}

fn main() {
    let mut args = std::env::args();
    args.next().unwrap();
    let path = args.next().unwrap();
    let path = Path::new(&path);
    let f = build_forest(&path);
    print(&f);
}
