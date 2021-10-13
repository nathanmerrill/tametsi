use std::{collections::HashMap, fs, path::PathBuf};

use roxmltree::Document;
use steamlocate::SteamDir;

use crate::{Bits, Puzzle, Square, SquareState};

const TAMETSI_APP_ID: u32 = 709920;


pub struct PuzzleListing {
    pub name: String,
    path: PathBuf,
}

impl PuzzleListing {
    pub fn new(path: PathBuf) -> Self {
        let contents = fs::read_to_string(path.clone())
            .expect(format!("Unable to read file: {}", path.to_string_lossy()).as_str());

        let doc = Document::parse(&contents).expect("Unable to parse XML!");
        if let Some(title_node) = doc.root().children().flat_map(|f| f.children()).find(|a| a.has_tag_name("TITLE")) {
            PuzzleListing {
                name: title_node.text().expect("No title given!").to_string(),
                path,
            }
        } else {
            println!("Length: {}", doc.root().children().count());
            println!("Tags: {}", doc.root().children().map(|a| a.tag_name().name()).collect::<Vec<_>>().join("\n"));
            panic!("No title in document! {}", path.to_string_lossy());
        }
    }

    pub fn read(&self) -> Puzzle {
        let contents = fs::read_to_string(self.path.clone())
            .expect(format!("Unable to read file: {}", self.path.to_string_lossy()).as_str());

        let doc = Document::parse(&contents).expect("Unable to parse XML!");

        let nodes = doc.root().children().flat_map(|f| f.children()).find(|a| a.has_tag_name("GRAPH")).expect("No graph in document!").children();

        let mut id_map = HashMap::new();
        let mut square_id = 0;
        let mut squares = Vec::new();
        let mut revealed = Bits::zeroed();
        let mut hints = Vec::new();

        for node in nodes.clone() {
            let id = node.children().find(|a| a.has_tag_name("ID")).and_then(|f|f.text()).expect("No ID in graph!");
            id_map.insert(id, square_id);
            square_id += 1;
        }

        for node in nodes {
            let neighbors = node.children().find(|a| a.has_tag_name("EDGES")).and_then(|f|f.text()).expect("No edges in graph!");
            let has_mine = node.children().any(|a| a.has_tag_name("HAS_MINE"));
            let secret = node.children().any(|a| a.has_tag_name("SECRET"));
            let is_revealed = node.children().any(|a| a.has_tag_name("REVEALED"));
            assert!(!has_mine || !secret, "Both HAS_MINE and SECRET were set!");
            let state = if has_mine {
                SquareState::Mine
            } else if secret {
                SquareState::Unknown
            } else {
                SquareState::Empty
            };
            let mut neighbor_map = Bits::zeroed();

            for neighbor in neighbors.split(',') {
                let neighbor_id = id_map[neighbor];
                neighbor_map.set(neighbor_id, true);
            }

            if is_revealed {
                revealed.set(squares.len(), true);
            }

            squares.push(Square{state, neighbors: neighbor_map});
        }

        
        for hint in doc.root().children().flat_map(|f| f.children()).filter(|a| a.has_tag_name("HINT_LIST") || a.has_tag_name("COLUMN_HINT_LIST")).flat_map(|a| a.children()) {
            let ids = hint.children().find(|a| a.has_tag_name("IDS")).and_then(|f|f.text()).expect("No ids in hint!");
            let mut bits = Bits::zeroed();
            for id in ids.split(",") {
                let square_id = id_map[id];
                bits.set(square_id, true);
            }

            hints.push(bits);
        }



        Puzzle {
            squares,
            revealed,
            hints
        }
    }
}

pub struct Parser {
    puzzle_dir: PathBuf,
}

impl Parser {
    pub fn new() -> Parser {
        if let Some(mut steamdir) = SteamDir::locate() {
            if let Some(app) = steamdir.app(&TAMETSI_APP_ID) {
                let mut path = app.path.clone();
                path.push("puzzles");
                Parser {
                    puzzle_dir: path
                }
            } else {
                panic!("Couldn't locate Tametsi on the computer!")
            }
        } else {
            panic!("Couldn't locate Steam on this computer!")
        }
    }

    pub fn from_folder<T>(path: T) -> Parser 
        where T: Into<PathBuf> + Sized
    {
        Parser { puzzle_dir: path.into() }
    }
    
    pub fn read_all_puzzles(&self) -> Vec<PuzzleListing> {
        let mut puzzles = Vec::new();
        for entry in fs::read_dir(self.puzzle_dir.clone()).expect("Unable to read puzzle directory!") {
            let entry = entry.expect("Unable to read puzzle directory!");
            let path = entry.path();
            if path.is_file() {
                puzzles.push(PuzzleListing::new(path));
            }
        }
        puzzles
    }    
}
