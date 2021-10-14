use std::{sync::mpsc::{self, Receiver, Sender}, thread};

use eframe::{egui::{self, Align2, Color32, Pos2, Shape, Stroke, TextStyle}, epi};

use crate::{core::{PuzzleGui}, parser::{Parser, PuzzleListing}, solver::{PuzzleState, Solver, StepResult}};

#[derive(PartialEq, Eq)]
pub enum Command {
    Run,
    Load(PuzzleListing),
    Step,
    Stop,
}

pub enum Update {
    PuzzleListing(Vec<PuzzleListing>),
    NewPuzzle(PuzzleState, PuzzleGui),
    Step(PuzzleState, StepResult),
}

pub fn start_engine(send: Sender<Update>, recieve: Receiver<Command>) {
    let parser = Parser::new();
    let puzzles = parser.read_all_puzzles();
    send.send(Update::PuzzleListing(puzzles)).unwrap();
    let mut solver = None;
    let mut running = false;
    loop {
        let command = if running {
            recieve.try_recv().unwrap_or(Command::Run)
        } else {
            recieve.recv().unwrap()
        };

        if command == Command::Run {
            running = true;
        } else {
            running = false;
        }

        match command {
            Command::Load(listing) => {
                let (puzzle, gui) = listing.read();
                let new_solver = Solver::new(puzzle, 3, 9);
                send.send(Update::NewPuzzle(new_solver.puzzle.clone(), gui)).unwrap();
                solver = Some(new_solver);
            }
            Command::Run | Command::Step => {
                if let Some(s) = solver.as_mut() {
                    let response = s.step();
                    match response {
                        StepResult::Finished | StepResult::UnexpectedStop(_) => {
                            running = false
                        }
                        _ => {}
                    }
                    send.send(Update::Step(s.puzzle.clone(), response)).unwrap();
                }
            }
            Command::Stop => {}
        }
    }
}

pub struct TemplateApp {
    step: usize,
    display_puzzle: bool,
    send: Sender<Command>,
    recieve: Receiver<Update>,
    listing: Vec<PuzzleListing>,
    puzzle: Option<PuzzleDisplay>,
}

pub struct PuzzleDisplay {
    starting_state: PuzzleState,
    gui: PuzzleGui,
    steps: Vec<(PuzzleState, StepResult)>
}

impl Default for TemplateApp {
    fn default() -> Self {
        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();

        thread::spawn(move || start_engine(tx1, rx2));

        Self {
            step: 0,
            send: tx2,
            recieve: rx1,
            listing: Vec::new(),
            puzzle: None,
            display_puzzle: false,
        }
    }
}

impl TemplateApp {
    fn recieve_updates(&mut self) {
        while let Ok(update) = self.recieve.try_recv() {
            match update {
                Update::NewPuzzle(state, gui) => {
                    self.puzzle = Some(PuzzleDisplay {
                        starting_state: state,
                        gui,
                        steps: vec![]
                    });
                    self.step = 0;
                    self.display_puzzle = true;
                }
                Update::PuzzleListing(listing) => {
                    self.listing = listing
                }
                Update::Step(state, result) => {
                    let display = self.puzzle.as_mut().expect("Not in a puzzle!");
                    if self.step == display.steps.len() {
                        self.step += 1;
                    }

                    display.steps.push((state, result));
                }
            }   
        }
    }
}

impl epi::App for TemplateApp {
    fn name(&self) -> &str {
        "Tametsi Generator"
    }

    /// Called once before the first frame.
    fn setup(
        &mut self,
        _ctx: &egui::CtxRef,
        _frame: &mut epi::Frame<'_>,
        _storage: Option<&dyn epi::Storage>
    ) {
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        self.recieve_updates();

        let Self {step, listing, puzzle, send, display_puzzle, .. } = self;

        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui
        if *display_puzzle {
            let puzzle_display = puzzle.as_mut().expect("No puzzle to display!");
            let (current_state, current_step) = match step {
                0 => (&puzzle_display.starting_state, None),
                _ => {
                    let step = &puzzle_display.steps[*step -1];
                    (&step.0, Some(&step.1))
                }
            };

            let sidebar_width = 200.0;

            

            egui::SidePanel::left("side_panel").min_width(sidebar_width).max_width(sidebar_width).resizable(false).show(ctx, |ui| {
                ui.heading("Control Panel");
                if ui.button("Back").clicked() {
                    *display_puzzle = false;
                    send.send(Command::Stop).unwrap();
                }

                ui.horizontal(|ui| {
                    if ui.button("<").clicked() {
                        *step = step.saturating_sub(1);
                    }

                    ui.add(egui::Slider::new(step, 0..=puzzle_display.steps.len()).text("Step"));

                    if ui.button(">").clicked() {
                        *step = puzzle_display.steps.len().min(1+*step);
                    }
                });

                ui.horizontal(|ui| {
                    if ui.button("Start").clicked() {
                        send.send(Command::Run).unwrap();
                    }
                    if ui.button("Step").clicked() {
                        send.send(Command::Step).unwrap();
                    }
                    if ui.button("Stop").clicked() {
                        send.send(Command::Stop).unwrap();
                    }
                });

                let text = match current_step.as_ref() {
                    None => String::new(),
                    Some(StepResult::CrossConstraint(c)) => format!("Crossing constraint.  Min: {} Max: {}", c.min_mines, c.max_mines),
                    Some(StepResult::Progress{revealed, flagged}) => {
                        if revealed.any() {
                            if flagged.any() {
                                format!("Found {} to be revealed and {} to be flagged", format_text(revealed.count_ones()), format_text(flagged.count_ones()))
                            } else {
                                format!("Found {} squares to be revealed", format_text(revealed.count_ones()))
                            }
                        } else {
                            format!("Found {} squares to be flagged", format_text(flagged.count_ones()))
                        }
                    }
                    Some(StepResult::Finished) => String::from("Finished!"),
                    Some(StepResult::UnexpectedStop(why)) => format!("Unexpected stop! Reason: {}", why),
                    Some(StepResult::CliqueConstraint(_)) => format!("Found maximal clique!  Adding remaining squares to constraint"),
                };

                ui.label(text)

            });
    
            egui::CentralPanel::default().show(ctx, |ui| {
                let margin = 50.0;
                let window_width = ui.available_width() - margin*2.0;
                let window_height = ui.available_height() - margin*2.0;
                
                let offset_x = puzzle_display.gui.min_x;
                let offset_y = puzzle_display.gui.min_y;
                let display_width = puzzle_display.gui.max_x - offset_x;
                let display_height = puzzle_display.gui.max_y - offset_y;
                let scale = (window_width/display_width).min(window_height/display_height);

                for (i, object) in puzzle_display.gui.squares.iter().enumerate() {
                    let (mut color, text) = if current_state.revealed[i] {
                        (Color32::GRAY, if current_state.base.unknowns[i] {
                            String::from("?")
                        } else {
                            (current_state.base.neighbors[i] & current_state.base.mines & !current_state.flagged).count_ones().to_string()
                        })
                    } else if current_state.flagged[i] {
                        (Color32::RED, String::new())
                    } else {
                        (Color32::BLUE, String::new())
                    };

                    let should_highlight = match current_step {
                        Some(StepResult::CrossConstraint(constraint)) => constraint.bits[i],
                        Some(StepResult::Progress{revealed, flagged}) => revealed[i] | flagged[i],
                        Some(StepResult::CliqueConstraint(constraint)) => constraint.bits[i],
                        _ => false,
                    };
                    
                    if !should_highlight {
                        color = color.linear_multiply(0.5)
                    }

                    let base_position_x = (object.x - offset_x)*scale + margin + sidebar_width + 50.0;
                    let base_position_y = (object.y - offset_y)*scale + margin;
                    ui.painter().add(Shape::Path {
                        points: object.points.iter().map(|a| Pos2 {
                            x: a.0*scale + base_position_x,
                            y: a.1*scale + base_position_y
                        }).collect(),
                        closed: true,
                        fill: color,
                        stroke: Stroke {
                            width: 1.0,
                            color: Color32::BLACK,
                        }
                    });
                    ui.painter().text( Pos2 { x: base_position_x, y: base_position_y }, Align2::CENTER_CENTER, text, TextStyle::Body, Color32::WHITE);
                }
            });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Puzzles");
                egui::ScrollArea::auto_sized().show(ui, |ui| {
                    for item in listing.iter().cloned() {
                        if ui.button(item.name.to_string()).clicked() {
                            send.send(Command::Load(item)).unwrap();
                        }
                    }
                });
            });
        }
    }
}

fn format_text(count: usize) -> String {
    if count != 1 {
        format!("{} squares", count)
    } else {
        String::from("1 square")
    }
}