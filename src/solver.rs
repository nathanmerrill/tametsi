use std::{collections::{HashMap, HashSet, VecDeque}, time::Instant};

use crate::{Bits, Puzzle, SquareState};


#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct Constraint {
    bits: Bits,
    min_mines: usize,
    max_mines: usize,
    size: usize,
}

impl Constraint {
    fn is_exact(self) -> bool {
        self.min_mines == self.max_mines
    }

    fn is_solved(self) -> bool {
        self.max_mines == 0 || self.min_mines == self.size
    }

    fn is_useless(self) -> bool {
        self.min_mines == 0 && self.max_mines == self.size
    }
}

struct PuzzleState {
    base: Puzzle,
    revealed: Bits,
    flagged: Bits,
}

pub struct Solver {
    puzzle: PuzzleState,
    unsolved: HashMap<Bits, Constraint>,
    processing_stack: VecDeque<Constraint>,
    square_constraints: Vec<HashSet<Constraint>>,
    solved: HashSet<Constraint>,
    max_cells: usize,
    max_mines: usize,
}

impl Solver {
    pub fn new(base: Puzzle, max_cells: usize, max_mines: usize) -> Solver {
        let revealed = base.revealed;

        let puzzle = PuzzleState {
            base,
            revealed: Bits::zeroed(),
            flagged: Bits::zeroed(),
        };      

        let mut square_constraints = Vec::new();
        square_constraints.resize(puzzle.base.squares.len(), HashSet::new());

        let mut solver = Solver {
            puzzle: puzzle,
            unsolved: HashMap::new(),
            solved: HashSet::new(),
            processing_stack: VecDeque::new(),
            square_constraints: square_constraints,
            max_cells,
            max_mines,
        };

        for hint in solver.puzzle.base.hints.clone() {
            let bits = hint & !revealed;
            let mine_count = bits.iter_ones().filter(|a| solver.puzzle.base.squares[*a].state == SquareState::Mine).count();
            solver.add_constraint(Constraint {
                bits,
                min_mines: mine_count,
                max_mines: mine_count,
                size: bits.count_ones(),
            });
        }

        for square in revealed.iter_ones() {
            solver.reveal_square(square);
        }

        solver
    }
    
    fn add_constraint(self: &mut Solver, constraint: Constraint) {
        assert!((constraint.bits & self.puzzle.revealed).not_any(), "Constraint involves revealed square! \nConstraint: {}, \nRevealed:   {}", constraint.bits.to_string(), self.puzzle.revealed.to_string());
        assert!((constraint.bits & self.puzzle.flagged).not_any(), "Constraint involves flagged square! \nConstraint: {}, \nFlagged:    {}", constraint.bits.to_string(), self.puzzle.flagged.to_string());

        if constraint.is_useless() {
            return;
        }

        if constraint.is_solved() {
            self.solved.insert(constraint);
            return;
        }

        if let Some(&known) = self.unsolved.get(&constraint.bits) {
            if known.min_mines >= constraint.min_mines && known.max_mines <= constraint.max_mines {
                return;
            }

            let new = Constraint {
                bits: constraint.bits,
                min_mines: known.min_mines.max(constraint.min_mines),
                max_mines: known.max_mines.min(constraint.max_mines),
                size: constraint.bits.count_ones(),
            };

            self.remove_constraint(known);
            self.add_constraint(new);
            return;
        }

        self.unsolved.insert(constraint.bits, constraint);
        if constraint.is_exact() {
            self.processing_stack.push_back(constraint);
        } else {
            self.processing_stack.push_front(constraint);
        }
        constraint.bits.iter_ones().for_each(|square| {self.square_constraints[square].insert(constraint);});

    }

    fn remove_constraint(self: &mut Solver, constraint: Constraint) {
        if constraint.is_solved() {
            return;
        }

        if let Some(known) = self.unsolved.remove(&constraint.bits) {
            for i in known.bits.iter_ones() {
                self.square_constraints[i].remove(&known);
            }        
        }
    }

    fn reveal_square(self: &mut Solver, square: usize) {
        assert!(!self.puzzle.revealed[square], "Square {} already revealed! \nRevealed:   {}", square, self.puzzle.revealed.to_string());
        assert!(self.puzzle.base.squares[square].state != SquareState::Mine, "Square {} was revealed, but was a mine!", square);

        for mut constraint in self.square_constraints[square].clone() {
            assert!(constraint.size > 0, "Revealed a square in a 0-sized constraint!");
            assert!(constraint.bits[square], "Constraint did not include target square!");
            
            self.remove_constraint(constraint);
            constraint.bits.set(square, false);
            constraint.size -= 1;
            self.add_constraint(constraint);
        }

        self.puzzle.revealed.set(square, true);

        if self.puzzle.base.squares[square].state == SquareState::Empty {
            self.add_constraint(get_neighbor_constraint(&self.puzzle, square))
        }
    }

    fn flag_square(self: &mut Solver, square: usize) {
        assert!(!self.puzzle.flagged[square], "Square {} already flagged! \nFlagged:   {}", square, self.puzzle.flagged.to_string());
        assert!(self.puzzle.base.squares[square].state == SquareState::Mine, "Flagged a non-mine!");
        
        for mut constraint in self.square_constraints[square].clone() {
            assert!(constraint.max_mines > 0, "Flagged a mine in a constraint with 0 max mines!");
            assert!(constraint.size > 0, "Flagged a mine in a constraint with a size of 0!");

            self.remove_constraint(constraint);
            constraint.bits.set(square, false);
            constraint.size -= 1;
            constraint.max_mines -= 1;
            constraint.min_mines = constraint.min_mines.saturating_sub(1);
            self.add_constraint(constraint);
        }

        self.puzzle.flagged.set(square, true);
    }

    fn add_all_crosses(self: &mut Solver, constraint: Constraint) {
        let mut seen = Bits::zeroed();
        let mut crosses = Vec::new();

        for square in constraint.bits.iter_ones() {
            for &to_cross in &self.square_constraints[square] {
                if to_cross.max_mines > self.max_mines && to_cross.size > self.max_cells
                {
                    continue;
                }

                if (to_cross.bits & seen).any() || constraint == to_cross {
                    continue;
                }

                crosses.extend(cross_constraints(constraint, to_cross))
            }

            seen.set(square, true)
        }

        for cross in crosses {
            self.add_constraint(cross);
        }
    }

    pub fn solve(&mut self) {
        let start = Instant::now();
        
        loop {
            if (self.puzzle.revealed | self.puzzle.flagged).count_ones() == self.puzzle.base.squares.len() {
                break;
            }

            if !self.solved.is_empty() 
            {
                let mut to_reveal = Bits::zeroed();
                let mut to_flag = Bits::zeroed();
                for &constraint in &self.solved {
                    if constraint.max_mines == 0 {
                        to_reveal |= constraint.bits;
                    } else {
                        to_flag |= constraint.bits;
                    }
                }

                self.solved.clear();

                for square in to_reveal.iter_ones() {
                    self.reveal_square(square);
                }
                
                for square in to_flag.iter_ones() {
                    self.flag_square(square);
                }

                continue;
            }

            if let Some(next) = self.processing_stack.pop_back() {
                if (next.bits & (self.puzzle.flagged | self.puzzle.revealed)).any() {
                    continue;
                }

                self.add_all_crosses(next);
            } else {
                panic!("No more constraints!")
            }
        }
        let duration = start.elapsed();

        println!("Solved puzzle in: {:?}", duration);
    }
}


fn get_neighbor_constraint(puzzle: &PuzzleState, square_index: usize) -> Constraint {
    let square = puzzle.base.squares[square_index];
    let unknown_neighbors = square.neighbors & !puzzle.revealed & !puzzle.flagged;
    let remaining_mines =  unknown_neighbors.iter_ones().filter(|neighbor| puzzle.base.squares[*neighbor].state == SquareState::Mine).count(); 

    Constraint {
        bits: unknown_neighbors,
        max_mines: remaining_mines,
        min_mines: remaining_mines,
        size: unknown_neighbors.count_ones(),
    }
}

fn cross_constraints(left: Constraint, right: Constraint) -> Vec<Constraint> {    
    let mut constraints = Vec::new();

    let intersection = left.bits & right.bits;
    let intersection_count = intersection.count_ones();
    let intersection_min = (left.min_mines + intersection_count).saturating_sub(left.size).max((right.min_mines + intersection_count).saturating_sub(right.size));
    let intersection_max = intersection_count.min(left.max_mines).min(right.max_mines);

    constraints.push(Constraint {
        bits: intersection,
        min_mines: intersection_min,
        max_mines: intersection_max,
        size: intersection.count_ones(),
    });
    

    let left_overlap = left.bits & !right.bits;
    if left_overlap.any() {
        let left_overlap_min = left.min_mines.saturating_sub(intersection_max);
        let left_overlap_max = left.max_mines.saturating_sub(intersection_min).min(left.size - intersection_count);
        constraints.push(Constraint {
            bits: left_overlap,
            min_mines: left_overlap_min,
            max_mines: left_overlap_max,
            size: left_overlap.count_ones(),
        })
    }
    

    let right_overlap = right.bits & !left.bits;
    if right_overlap.any() {
        let right_overlap_min = right.min_mines.saturating_sub(intersection_max);

        let right_overlap_max = right.max_mines.saturating_sub(intersection_min).min(right.size - intersection_count);
        constraints.push(Constraint {
            bits: right_overlap,
            min_mines: right_overlap_min,
            max_mines: right_overlap_max,
            size: right_overlap.count_ones(),
        })
    }

    constraints
}