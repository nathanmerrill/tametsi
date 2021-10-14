use std::{collections::{HashMap, HashSet, VecDeque}};

use crate::core::{Bits, Puzzle, bits_to_string};


#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub struct Constraint {
    pub bits: Bits,
    pub min_mines: usize,
    pub max_mines: usize,
    pub size: usize,
}

impl Constraint {
    fn to_string(self, len: usize) -> String {
        format!("{} {}->{}/{}", bits_to_string(self.bits, len), self.min_mines, self.max_mines, self.size)
    }

    fn is_solved(self) -> bool {
        self.max_mines == 0 || self.min_mines == self.size
    }

    fn is_useless(self) -> bool {
        self.min_mines == 0 && self.max_mines == self.size
    }
}

#[derive(Clone)]
pub struct PuzzleState {
    pub base: Puzzle,
    pub revealed: Bits,
    pub flagged: Bits,
}

impl ToString for PuzzleState {
    fn to_string(&self) -> String {
        let mut line = String::new();
        line.push('[');
        for i in 0..self.base.size() {
            if self.revealed[i] {
                if self.base.unknowns[i] {
                    line.push('?');
                } else {
                    line.push(' ');
                }
            } else if self.flagged[i] {
                line.push('*')
            } else {
                line.push('.')
            }
        }
        line.push(']');
        line
    }
}

pub struct Solver {
    pub puzzle: PuzzleState,
    unsolved_cliques: Vec<(Bits, HashSet<Bits>, HashSet<Bits>)>,
    unsolved: HashMap<Bits, Constraint>,
    processing_stack: Vec<Vec<VecDeque<Constraint>>>,
    square_constraints: Vec<HashSet<Constraint>>,
    removed: HashSet<Constraint>,
    solved: HashSet<Constraint>,
    all_bits: Bits,
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
        square_constraints.resize(puzzle.base.size(), HashSet::new());
        
        let mut processing_stack = vec![];
        let mut sub_processing_stack = vec![];
        sub_processing_stack.resize(puzzle.base.size(), VecDeque::new());
        processing_stack.resize(puzzle.base.size(), sub_processing_stack);

        let mut all_bits = Bits::zeroed();
        for i in 0 .. puzzle.base.neighbors.len() {
            all_bits.set(i, true);
        }

        let mut solver = Solver {
            unsolved_cliques: vec![(Bits::zeroed(), puzzle.base.hints.iter().copied().collect(), HashSet::new())],
            all_bits,
            puzzle,
            unsolved: HashMap::new(),
            solved: HashSet::new(),
            removed: HashSet::new(),
            processing_stack,
            square_constraints,
            max_cells,
            max_mines,
        };
        
        let mut initial_constraints = HashSet::new();

        for hint in solver.puzzle.base.hints.clone() {
            let bits = hint & !revealed;
            initial_constraints.insert(bits);
        }

        
        if initial_constraints.len() == 0 {        
            solver.add_constraint_from_mine_count(all_bits);
        }      

        for square in revealed.iter_ones() {
            solver.reveal_square(square);
        }

        solver
    }

    fn find_cliques(&mut self) -> Option<Bits> {
        loop {
            if let Some((mut clique, mut remaining, mut excluded)) = self.unsolved_cliques.pop() {
                loop {
                    if remaining.is_empty() && excluded.is_empty() {
                        if clique != self.all_bits {
                            return Some(clique)
                        } else {
                            break;
                        }
                    }

                    if let Some(&constraint) = remaining.iter().next() {
                        if (constraint & clique).any() {
                            panic!("Not disjoint!")
                        }

                        let union = constraint | clique;

                        let new_remaining = remaining.iter().copied().filter(|p| (*p & union).not_any()).collect();
                        let new_excluded = excluded.iter().copied().filter(|p| (*p & union).not_any()).collect();

                        remaining.remove(&constraint);
                        excluded.insert(constraint);
                        self.unsolved_cliques.push((clique, remaining, excluded));

                        clique = union;
                        remaining = new_remaining;
                        excluded = new_excluded;
                    } else {
                        break;
                    }
                }
            } else {
                return None
            }
        }
    }

    fn add_constraint_from_mine_count(self: &mut Solver, bits: Bits) -> Constraint {
        let bits = bits & !self.puzzle.revealed & !self.puzzle.flagged;
        let mines = (bits & self.puzzle.base.mines).count_ones();
        let constraint = Constraint {
            bits,
            min_mines: mines,
            max_mines: mines,
            size: bits.count_ones()
        };
        self.add_constraint(constraint);

        constraint
    }
    
    fn add_constraint(self: &mut Solver, constraint: Constraint) {
        assert!((constraint.bits & self.puzzle.revealed).not_any(), "Constraint involves revealed square! \nConstraint: {}, \nPuzzle:   {}", constraint.to_string(self.puzzle.base.size()), self.puzzle.to_string());
        assert!((constraint.bits & self.puzzle.flagged).not_any(), "Constraint involves flagged square! \nConstraint: {}, \nPuzzle:    {}", constraint.bits.to_string(), self.puzzle.to_string());
        assert!(constraint.max_mines <= constraint.size, "Constraint has more max mines than its size! Constraint: {}", constraint.to_string(self.puzzle.base.size()));
        
        if constraint.is_useless() {
            return;
        }

        if let Some(&known) = self.unsolved.get(&constraint.bits) {
            assert!(constraint.bits == known.bits, "Constraint bits don't match known bits! \nConstraint: {}, \nKnown:   {}", constraint.to_string(self.puzzle.base.size()), known.to_string(self.puzzle.base.size()));
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

        if constraint.is_solved() {
            self.solved.insert(constraint);
        } else {
            self.unsolved.insert(constraint.bits, constraint);
            self.processing_stack[constraint.size-1][constraint.max_mines - constraint.min_mines].push_back(constraint);
        }

        constraint.bits.iter_ones().for_each(|square| {self.square_constraints[square].insert(constraint);});
    }

    fn remove_constraint(self: &mut Solver, constraint: Constraint) {
        let known = if constraint.is_solved() {
            if self.solved.remove(&constraint) {
                constraint
            } else {
                panic!("Constraint not in solved: {}", constraint.to_string(self.puzzle.base.size()))
            }
        } else {
            self.removed.insert(constraint);
            self.unsolved.remove(&constraint.bits).expect("Attempted to remove constraint that did not exist!")
        };

        for i in known.bits.iter_ones() {
            let removed = self.square_constraints[i].remove(&known);
            assert!(removed, "Attempted to remove constraint from a square that did not exist!");
        }  
    }

    fn reveal_square(self: &mut Solver, square: usize) {
        assert!(!self.puzzle.revealed[square], "Square {} already revealed! \nPuzzle:   {}", square, self.puzzle.to_string());
        assert!(!self.puzzle.base.mines[square], "Square {} was revealed, but was a mine!", square);

        for mut constraint in self.square_constraints[square].clone() {
            assert!(constraint.size > 0, "Revealed a square in a 0-sized constraint!");
            assert!(constraint.bits[square], "Constraint did not include target square!");
            
            self.remove_constraint(constraint);
            constraint.bits.set(square, false);
            constraint.size -= 1;
            constraint.max_mines = constraint.max_mines.min(constraint.size);
            self.add_constraint(constraint);
        }

        self.puzzle.revealed.set(square, true);

        if !self.puzzle.base.unknowns[square] {
            self.add_constraint(get_neighbor_constraint(&self.puzzle, square))
        }
    }

    fn flag_square(self: &mut Solver, square: usize) {
        assert!(!self.puzzle.flagged[square], "Square {} already flagged! \nPuzzle:   {}", square, self.puzzle.to_string());
        assert!(self.puzzle.base.mines[square], "Flagged a non-mine!");
        
        for mut constraint in self.square_constraints[square].clone() {
            assert!(constraint.max_mines > 0, "Flagged a mine in a constraint with 0 max mines!");
            assert!(constraint.size > 0, "Flagged a mine in a constraint with a size of 0!");
            assert!(constraint.bits[square], "Constraint did not include target square!");

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

    pub fn step(&mut self) -> StepResult {
        /* 
        if let Some(clique) = self.find_cliques() {
            let constraint = self.add_constraint_from_mine_count(!clique & self.all_bits);
            return StepResult::CliqueConstraint(constraint)
        }*/

        if !self.solved.is_empty() 
        {
            let mut to_reveal = Bits::zeroed();
            let mut to_flag = Bits::zeroed();
            for &constraint in &self.solved {
                assert!(constraint.size > 0, "Constraint of size 0 in solved!");
                if constraint.max_mines == 0 {
                    to_reveal |= constraint.bits;
                } else {
                    to_flag |= constraint.bits;
                }
            }

            assert!((to_flag & self.puzzle.revealed).not_any(), "Revealing existing squares! \nSquares:  {}\nPuzzle: {}\nConstraints: \n{}", bits_to_string(to_reveal, self.puzzle.base.size()), self.puzzle.to_string(), self.solved.iter().map(|c| c.to_string(self.puzzle.base.size())).collect::<Vec<String>>().join("\n"));
            assert!((to_flag & self.puzzle.flagged).not_any(), "Flagging existing flags! \nFlags:    {}\nExisting: {}\nConstraints: {}", bits_to_string(to_flag, self.puzzle.base.size()), self.puzzle.to_string(), self.solved.iter().map(|c| c.to_string(self.puzzle.base.size())).collect::<Vec<String>>().join("\n"));

            for square in to_reveal.iter_ones() {
                //println!("Revealing squares: {}", to_reveal);
                self.reveal_square(square);
            }
            
            for square in to_flag.iter_ones() {
                //println!("Flagging squares: {}", to_flag);
                self.flag_square(square);
            }
            let remaining = self.puzzle.base.size() - (self.puzzle.revealed | self.puzzle.flagged).count_ones();
            if remaining == 0 {
                return StepResult::Finished;
            }

            return StepResult::Progress {
                revealed: to_reveal,
                flagged: to_flag,
            };
        }
        loop {
            if let Some(next) = self.processing_stack.iter_mut().flatten().find_map(|f| f.pop_back()) {
                if !self.removed.remove(&next) {
                    self.add_all_crosses(next);
                    return StepResult::CrossConstraint(next);
                }
            } else {
                // This can happen if a previous constraint combination was ignored due to size
                for constraint in self.puzzle.base.hints.clone() {
                    self.add_constraint_from_mine_count(constraint);
                }

                for neighborhood in self.puzzle.base.neighbors.clone(){
                    self.add_constraint_from_mine_count(neighborhood);
                }
            }
        }
    }
}

pub enum StepResult {
    Progress{revealed: Bits, flagged: Bits},
    CrossConstraint(Constraint),
    CliqueConstraint(Constraint),
    UnexpectedStop(String),
    Finished,
}

fn get_neighbor_constraint(puzzle: &PuzzleState, square_index: usize) -> Constraint {
    let unknown_neighbors = puzzle.base.neighbors[square_index] & !puzzle.revealed & !puzzle.flagged;
    let remaining_mines =  (unknown_neighbors & puzzle.base.mines).count_ones();

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