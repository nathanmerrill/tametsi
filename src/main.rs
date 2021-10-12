use std::{collections::{HashMap, HashSet}, time::Instant};
use bitvec::prelude::*;

type Bits = BitArray<Lsb0, [usize; 1]>;

struct Puzzle {
    squares: Vec<Square>,
    revealed: Bits,
    flagged: Bits,
    color_constraints: Vec<Constraint>, 
}

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

    fn is_trivial(self) -> bool {
        self.max_mines == 0 || self.min_mines == self.size
    }

    fn is_useless(self) -> bool {
        self.min_mines == 0 && self.max_mines == self.size
    }
}


#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct Square {
    state: SquareState,
    neighbors: Bits,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum SquareState {
    Empty, 
    Mine,
    Unknown
}

struct Solver {
    puzzle: Puzzle,
    constraints: HashMap<Bits, Constraint>,
    square_constraints: Vec<HashSet<Constraint>>,
    trivial: HashSet<Constraint>,
    exact: HashSet<Constraint>,
    inexact: HashSet<Constraint>,
    max_cells: usize,
    max_mines: usize,
    max_inexact_stages: usize,
}

impl Solver {
    fn new(puzzle: Puzzle, max_cells: usize, max_mines: usize, max_inexact_stages: usize) -> Solver {
        let mut square_constraints = Vec::new();
        square_constraints.resize(puzzle.squares.len(), HashSet::new());

        let mut solver = Solver {
            square_constraints,
            puzzle: puzzle,
            constraints: HashMap::new(),
            trivial: HashSet::new(),
            exact: HashSet::new(),
            inexact: HashSet::new(),
            max_cells,
            max_mines,
            max_inexact_stages,
        };

        for constraint in solver.puzzle.color_constraints.clone() {
            solver.add_constraint(constraint);
        }

        let revealed = solver.puzzle.revealed;

        for square in revealed.iter_ones() {
            solver.reveal_square(square);
        }

        solver
    }
    
    fn add_constraint(self: &mut Solver, constraint: Constraint) {
        if constraint.is_useless() {
            return;
        }

        let new_constraint = if let Some(known) = self.constraints.get(&constraint.bits).copied() {
            if known.min_mines >= constraint.min_mines && known.max_mines <= constraint.max_mines {
                return;
            }

            self.remove_constraint(known);

            Constraint {
                bits: constraint.bits,
                min_mines: known.min_mines.max(constraint.min_mines),
                max_mines: known.max_mines.min(constraint.max_mines),
                size: constraint.bits.count_ones(),
            }
        } else {
            constraint
        };

        self.constraints.insert(new_constraint.bits, new_constraint);
        
        new_constraint.bits.iter_ones().for_each(|square| {
            self.square_constraints.get_mut(square).map(|s| s.insert(new_constraint));
        });

        if new_constraint.is_trivial() {
            self.trivial.insert(new_constraint);
        } else if new_constraint.is_exact() {
            self.exact.insert(new_constraint);
        } else {
            self.inexact.insert(new_constraint);
        }
    }

    fn remove_constraint(self: &mut Solver, constraint: Constraint) {
        if let Some(known) = self.constraints.remove(&constraint.bits) {
            if known.is_trivial() {
                self.trivial.remove(&known);
            } else if known.is_exact() {
                self.exact.remove(&known);
            } else {
                self.inexact.remove(&known);
            }

            for i in known.bits.iter_ones() {
                self.square_constraints.get_mut(i).map(|s| s.remove(&constraint));
            }
        }
    }

    fn reveal_square(self: &mut Solver, square: usize) {
        for mut constraint in self.square_constraints[square].clone(){
            if constraint.size == 0 {
                panic!("Impossibility detected when revealing a square!")
            }
            self.remove_constraint(constraint);
            constraint.bits.set(square, false);
            constraint.size -= 1;
            self.add_constraint(constraint);
        }

        self.puzzle.revealed.set(square, true);

        get_neighbor_constraint(&self.puzzle, square).map(|c|self.add_constraint(c));
    }

    fn flag_square(self: &mut Solver, square: usize) {
        if self.puzzle.squares[square].state != SquareState::Mine {
            panic!("Flagged a non-mine!")
        } 

        for mut constraint in self.square_constraints[square].clone(){
            if constraint.max_mines == 0 || constraint.size == 0 {
                panic!("Impossibility detected when flagging a square!")
            }

            self.remove_constraint(constraint);
            constraint.bits.set(square, false);
            constraint.size -= 1;
            constraint.max_mines -= 1;
            constraint.min_mines = constraint.min_mines.saturating_sub(1);
            self.add_constraint(constraint);
        }

        self.puzzle.flagged.set(square, true);
    }

    fn cross_all_pairs(self: &Solver, left_constraints: &HashSet<Constraint>, right_constraints: &HashSet<Constraint>) -> Vec<Constraint> {
        let mut seen = Bits::zeroed();
        let mut crosses = Vec::new();
        for (square, constraints) in self.square_constraints.iter().enumerate() {
            for left in constraints {
                if left.max_mines > self.max_mines && left.size > self.max_cells
                    || !left_constraints.contains(left) 
                {
                    continue;
                }

                for right in constraints {
                    if right.max_mines > self.max_mines && right.size > self.max_cells
                        || !right_constraints.contains(left) 
                    {
                        continue;
                    }

                    if left == right || (left.bits & right.bits & seen).any() {
                        continue;
                    }

                    crosses.extend(cross_constraints(*left, *right))

                }
            }

            seen.set(square, true)
        }

        crosses
    }

    fn solve(&mut self) {
        let start = Instant::now();
        
        loop {
            if (self.puzzle.revealed | self.puzzle.flagged).count_ones() == self.puzzle.squares.len() {
                break;
            }

            if !self.trivial.is_empty() 
            {
                let mut to_reveal = Bits::zeroed();
                let mut to_flag = Bits::zeroed();
                for constraint in self.trivial.iter() {
                    if constraint.max_mines == 0 {
                        to_reveal |= constraint.bits;
                    } else {
                        to_flag |= constraint.bits;
                    }
                }

                for square in to_reveal.iter_ones() {
                    self.reveal_square(square);
                }
                
                for square in to_flag.iter_ones() {
                    self.flag_square(square);
                }

                continue;
            }

            if !self.exact.is_empty() {
                let mut crossed = self.cross_all_pairs(&self.exact, &self.exact);
                crossed.extend(self.cross_all_pairs(&self.exact, &self.inexact));

                self.exact.clear();
                for constraint in crossed {
                    self.add_constraint(constraint)
                }

                continue;
            }

            if !self.inexact.is_empty() {
                if self.max_inexact_stages == 0 {
                    panic!("Unable to solve puzzle!  Too many inexact stages")
                }

                self.max_inexact_stages -= 1;

                let crossed = self.cross_all_pairs(&self.inexact, &self.inexact);
                self.inexact.clear();

                for constraint in crossed {
                    self.add_constraint(constraint)
                }
            } else {
                println!("{}", self.puzzle.revealed);
                println!("{}", self.puzzle.flagged);
                
                panic!("No more constraints!")
            }
        }
        let duration = start.elapsed();

        println!("Solved puzzle in: {:?}", duration);

    }

}


fn get_neighbor_constraint(puzzle: &Puzzle, square_index: usize) -> Option<Constraint> {
    if !puzzle.revealed[square_index] {
        None
    } else {
        let square = puzzle.squares[square_index];
        match square.state {
            SquareState::Mine => panic!("Mine revealed!"),
            SquareState::Unknown => None,
            SquareState::Empty => {
                let unknown_neighbors = square.neighbors & !puzzle.revealed & !puzzle.flagged;
                let remaining_mines =  unknown_neighbors.iter_ones().filter(|neighbor| puzzle.squares[*neighbor].state == SquareState::Mine).count(); 
                

                Some(Constraint {
                    bits: unknown_neighbors,
                    max_mines: remaining_mines,
                    min_mines: remaining_mines,
                    size: unknown_neighbors.count_ones(),
                })
            }
        }
    }
}

fn cross_constraints(left: Constraint, right: Constraint) -> Vec<Constraint> {
    
    let mut constraints = Vec::new();

    //Intersection
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




fn main() {
    let p = Puzzle {
        squares: vec![
            Square {
                state: SquareState::Unknown,
                neighbors: bitarr![0,1,0,1,0,1,1,0,0,0,0,0,0,0,0,0,0]
            },
            Square {
                state: SquareState::Unknown,
                neighbors: bitarr![1,0,1,1,1,0,0,0,0,0,0,0,0,0,0,0,0]
            },
            Square {
                state: SquareState::Unknown,
                neighbors: bitarr![0,1,0,0,1,0,0,1,1,0,0,0,0,0,0,0,0]
            },
            Square {
                state: SquareState::Mine,
                neighbors: bitarr![1,1,1,0,1,0,1,1,0,1,1,0,0,0,0,0,0]
            },
            Square {
                state: SquareState::Empty,
                neighbors: bitarr![0,1,1,1,0,0,0,1,0,0,0,0,0,0,0,0,0]
            },
            Square {
                state: SquareState::Empty,
                neighbors: bitarr![1,0,0,0,0,0,1,0,0,1,0,0,0,1,0,0,0]
            },
            Square {
                state: SquareState::Unknown,
                neighbors: bitarr![1,0,0,1,0,1,0,0,0,1,0,0,0,0,0,0,0]
            },
            Square {
                state: SquareState::Empty,
                neighbors: bitarr![0,0,1,1,1,0,0,0,1,0,1,1,1,0,0,0,0]
            },
            Square {
                state: SquareState::Empty,
                neighbors: bitarr![0,0,1,0,0,0,0,1,0,0,0,0,1,0,0,1,0]
            },
            Square {
                state: SquareState::Empty,
                neighbors: bitarr![0,0,0,1,0,1,1,0,0,0,1,1,0,1,1,0,0]
            },
            Square {
                state: SquareState::Unknown,
                neighbors: bitarr![0,0,0,1,0,0,0,1,0,1,0,1,0,0,0,0,0]
            },
            Square {
                state: SquareState::Empty,
                neighbors: bitarr![0,0,0,0,0,0,0,1,0,1,1,0,1,0,1,1,1]
            },
            Square {
                state: SquareState::Mine,
                neighbors: bitarr![0,0,0,0,0,0,0,1,1,0,0,1,0,0,0,1,0]
            },
            Square {
                state: SquareState::Mine,
                neighbors: bitarr![0,0,0,0,0,1,0,0,0,1,0,0,0,0,1,0,1]
            },
            Square {
                state: SquareState::Unknown,
                neighbors: bitarr![0,0,0,0,0,0,0,0,0,1,0,1,0,1,0,0,1]
            },
            Square {
                state: SquareState::Unknown,
                neighbors: bitarr![0,0,0,0,0,0,0,0,1,0,0,1,1,0,0,0,1]
            },
            Square {
                state: SquareState::Empty,
                neighbors: bitarr![0,0,0,0,0,0,0,0,0,0,0,1,0,1,1,1,0]
            },
        ],
        revealed: bitarr![0,0,0,0,0,0,0,0,0,0,1,1,0,0,0,0,1],
        flagged: bitarr![0; 17],
        color_constraints: vec![
            Constraint {
                bits: bitarr![1,0,1,0,0,0,0,0,0,0,0,0,0,1,0,1,0],
                min_mines: 1,
                max_mines: 1,
                size: 4
            },
            Constraint {
                bits: bitarr![0,1,0,0,0,1,0,0,1,0,0,0,0,0,0,0,1],
                min_mines: 0,
                max_mines: 0,
                size: 4
            },
            Constraint {
                bits: bitarr![0,0,0,1,0,0,0,1,0,1,0,1,0,0,0,0,0],
                min_mines: 1,
                max_mines: 1,
                size: 4
            },
            Constraint {
                bits: bitarr![0,0,0,0,1,0,1,0,0,0,1,0,1,0,1,0,0],
                min_mines: 1,
                max_mines: 1,
                size: 5
            },
            Constraint {
                bits: bitarr![1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1],
                min_mines: 3,
                max_mines: 3,
                size: 17
            },
        ]
    };

    let mut solver = Solver::new(p, 9, 3, 100);
    solver.solve()
}
