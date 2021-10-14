
use bitvec::prelude::*;

pub type Bits = BitArray<Lsb0, [usize; 7]>;

#[derive(Clone)]
pub struct Puzzle {
    pub neighbors: Vec<Bits>,
    pub mines: Bits,
    pub unknowns: Bits,
    pub revealed: Bits,
    pub hints: Vec<Bits>,
}

pub struct PuzzleGui {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub squares: Vec<SquareDimensions>
}

pub struct SquareDimensions {
    pub x: f32,
    pub y: f32,
    pub points: Vec<(f32, f32)>,
}

impl Puzzle {
    #[inline]
    pub fn size(&self) -> usize {
        self.neighbors.len()
    }
}

impl ToString for Puzzle {
    fn to_string(&self) -> String {
        let mut line = String::new();
        line.push('[');
        for i in 0..self.size() {
            line.push(if self.mines[i] {
                '*'
            } else if self.unknowns[i] {
                '?'
            } else {
                ' '
            });
        }
        line.push(']');
        line
    }
}

pub fn bits_to_string(bits: Bits, len: usize) -> String {
    let mut line = String::new();
    line.push('[');
    for i in 0..len {
        line.push(if bits[i] {
            'X'
        } else {
            ' '
        });
    }
    line.push(']');
    line
}