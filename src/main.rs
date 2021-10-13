
mod solver;
mod parser;

use bitvec::prelude::*;

type Bits = BitArray<Lsb0, [usize; 7]>;

pub struct Puzzle {
    squares: Vec<Square>,
    revealed: Bits,
    hints: Vec<Bits>, 
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct Square {
    state: SquareState,
    neighbors: Bits,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum SquareState {
    Empty, 
    Mine,
    Unknown
}


fn main() {
    let parser = parser::Parser::new();
    for listing in parser.read_all_puzzles() {
        println!("Solving puzzle {}", listing.name);
        solver::Solver::new(listing.read(), 9, 3).solve();
    }
}
