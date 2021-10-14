#![forbid(unsafe_code)]


mod solver;
mod parser;
mod core;
mod app;


// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let app = app::TemplateApp::default();
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(Box::new(app), native_options);
}

/*
fn main() {
    let parser = parser::Parser::new();
    for listing in parser.read_all_puzzles() {
        println!("Solving puzzle {}", listing.name);
        solver::Solver::new(listing.read(), 9, 3).solve();
    }
} */
