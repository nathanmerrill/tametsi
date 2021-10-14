use std::{collections::HashMap, fs, path::PathBuf};

use roxmltree::Document;
use steamlocate::SteamDir;

use crate::core::{Bits, Puzzle, PuzzleGui, SquareDimensions};

const TAMETSI_APP_ID: u32 = 709920;

#[derive(PartialEq, Eq, Clone)]
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

    pub fn read(&self) -> (Puzzle, PuzzleGui) {
        let contents = fs::read_to_string(self.path.clone())
            .expect(format!("Unable to read file: {}", self.path.to_string_lossy()).as_str());

        let doc = Document::parse(&contents).expect("Unable to parse XML!");

        let nodes = doc.root().children().flat_map(|f| f.children()).find(|a| a.has_tag_name("GRAPH")).expect("No graph in document!").children();

        let mut id_map = HashMap::new();
        let mut revealed = Bits::zeroed();
        let mut mines = Bits::zeroed();
        let mut unknowns = Bits::zeroed();
        let mut hints = Vec::new();
        let mut neighbors = Vec::new();
        let mut square_dimensions= Vec::new();

        for node in nodes.clone() {
            let id = node.children().find(|a| a.has_tag_name("ID")).and_then(|f|f.text()).expect("No ID in graph!");
            id_map.insert(id, id_map.len());
            neighbors.push(Bits::zeroed());
            square_dimensions.push(SquareDimensions {
                x: 0.0,
                y: 0.0,
                points: vec![],
            })
        }

        for node in nodes {
            let id = node.children().find(|a| a.has_tag_name("ID")).and_then(|f|f.text()).expect("No ID in graph!");
            let index = id_map[&id];
            let edges = node.children().find(|a| a.has_tag_name("EDGES")).and_then(|f|f.text()).unwrap_or("");
            let has_mine = node.children().any(|a| a.has_tag_name("HAS_MINE"));
            let secret = node.children().any(|a| a.has_tag_name("SECRET"));
            let is_revealed = node.children().any(|a| a.has_tag_name("REVEALED"));
            assert!(!has_mine || !secret, "Both HAS_MINE and SECRET were set!");
            let pos = node.children().find(|a| a.has_tag_name("POS")).and_then(|f|f.text()).expect("No POS in node!").split(',').map(|a| a.parse::<f32>().expect("Unable to parse float!")).collect::<Vec<_>>();
            let (x, y) = (pos[0], pos[1]);

            let points = node.children().find(|a| a.has_tag_name("POLY")).expect("No POLY in node!").children().find(|a| a.has_tag_name("POINTS")).and_then(|f|f.text()).expect("No POINTS in node!").split(",").map(|a| a.parse::<f32>().expect("Unable to parse float!")).collect::<Vec<_>>();
            let mut points_iter = points.into_iter();
            while let Some(first) = points_iter.next() {
                let second = points_iter.next().expect("Elements in POINTS are not paired!");

                square_dimensions[index].points.push((first, second));
            }
            square_dimensions[index].x = x;
            square_dimensions[index].y = y;


            
            let mut neighbor_map = Bits::zeroed();

            if edges != "" {
                for edge in edges.split(',') {
                    let neighbor_id = id_map[edge];
                    neighbor_map.set(neighbor_id, true);
                }
            }

            revealed.set(index, is_revealed);
            mines.set(index, has_mine);
            unknowns.set(index, secret);

            neighbors[index] = neighbor_map;
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

        let min_x = square_dimensions.iter().map(|a| a.x).reduce(f32::min).unwrap();
        let max_x = square_dimensions.iter().map(|a| a.x).reduce(f32::max).unwrap();
        let min_y = square_dimensions.iter().map(|a| a.y).reduce(f32::min).unwrap();
        let max_y = square_dimensions.iter().map(|a| a.y).reduce(f32::max).unwrap();

        (
            Puzzle {
                neighbors,
                revealed,
                hints,
                mines,
                unknowns
            },
            PuzzleGui {
                min_y,
                min_x,
                max_y,
                max_x, 
                squares: square_dimensions,
            }
        )
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
