use cgmath::{abs_diff_eq, Vector2};

pub mod render;
use render::*;

pub const MAX_STONES: u64 = 1024;

enum StoneType {
    Empty,
    Black,
    White,
}

struct BoardPoint {
    pos: Vector2<f32>,
    neighbors: Vec<i32>,
    ty: StoneType,
}

struct Board {
    points: Vec<BoardPoint>,
}

impl Board {
    fn make_square(width: u32, height: u32) -> Board {
        let mut board = Board { points: Vec::new() };
        let pos_offset = Vector2::new(width as f32, height as f32) / 2.0;

        for r in 0..height {
            for c in 0..width {
                let mut point = BoardPoint {
                    pos: Vector2::new(c as f32, r as f32) - pos_offset,
                    neighbors: Vec::new(),
                    ty: StoneType::Empty,
                };

                // horrendous
                for r2 in -1..2 {
                    for c2 in -1..2 {
                        let i = board.find_point(Vector2::new(c2 as f32, r2 as f32) + point.pos);
                        if i >= 0 {
                            point.neighbors.push(i);
                        }
                    }
                }
                board.points.push(point);
            }
        }

        board
    }

    fn find_point(&self, pos: Vector2<f32>) -> i32 {
        for (i, point) in self.points.iter().enumerate() {
            if abs_diff_eq!(point.pos, pos) {
                return i as i32;
            }
        }
        -1
    }
}

enum Turn {
    Black,
    White,
}

pub struct GameState {
    board: Board,
    turn: Turn,
}

impl GameState {
    pub fn new() -> Self {
        let board = Board::make_square(3, 2);
        Self {
            board,
            turn: Turn::Black,
        }
    }
}
