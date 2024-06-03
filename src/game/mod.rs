use cgmath::{abs_diff_eq, MetricSpace, Vector2};

pub mod render;
use render::*;

pub const MAX_STONES: u64 = 1024;
pub const STONE_RADIUS: f32 = 0.4;

#[derive(PartialEq)]
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
    links: Vec<(i32, i32)>,
}

impl Board {
    fn make_square(width: u32, height: u32) -> Board {
        let mut board = Board {
            points: Vec::new(),
            links: Vec::new(),
        };
        let pos_offset = Vector2::new(width as f32, height as f32) / 2.0;

        for r in 0..height {
            for c in 0..width {
                let mut point = BoardPoint {
                    pos: Vector2::new(c as f32, r as f32) - pos_offset,
                    neighbors: Vec::new(),
                    ty: StoneType::Empty,
                };
                let this_idx = board.points.len() as i32;

                // horrendous
                for r2 in -1..2i32 {
                    for c2 in -1..2i32 {
                        if r2.abs() + c2.abs() != 1 {
                            continue;
                        }
                        let i =
                            board.find_point(Vector2::new(c2 as f32, r2 as f32) + point.pos, 0.1);
                        if i >= 0 {
                            point.neighbors.push(i);
                            board.points[i as usize].neighbors.push(this_idx);
                            board.links.push((i, this_idx));
                        }
                    }
                }
                board.points.push(point);
            }
        }

        board
    }

    fn find_point(&self, pos: Vector2<f32>, dist: f32) -> i32 {
        let dist2 = dist * dist;
        for (i, point) in self.points.iter().enumerate() {
            if pos.distance2(point.pos) <= dist2 {
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
    pub needs_render: bool,
}

impl GameState {
    pub fn new() -> Self {
        let board = Board::make_square(9, 9);
        Self {
            board,
            turn: Turn::Black,
            needs_render: true,
        }
    }

    fn update_captures(&mut self, point_idx: i32) -> bool {
        let captured_type = match self.turn {
            Turn::Black => StoneType::White,
            Turn::White => StoneType::Black,
        };
        let mut captured_idxs = vec![];

        let start_point = &self.board.points[point_idx as usize];
        'outer: for start_idx in start_point.neighbors.iter() {
            // redundant but skips allocs if no potential to capture
            if self.board.points[*start_idx as usize].ty != captured_type {
                continue;
            }
            let mut search_stack = vec![*start_idx];
            let mut checked_idxs = vec![];

            while let Some(i) = search_stack.pop() {
                let point = &self.board.points[i as usize];
                match point.ty {
                    StoneType::Empty => continue 'outer,
                    _ => {
                        if point.ty == captured_type && checked_idxs.iter().all(|&x| x != i) {
                            search_stack.extend(point.neighbors.iter());
                            checked_idxs.push(i);
                        }
                    }
                }
            }

            // capture success if we make it here
            captured_idxs.append(&mut checked_idxs);
        }

        for i in captured_idxs.iter() {
            self.board.points[*i as usize].ty = StoneType::Empty;
            // TODO scoring?
        }

        !captured_idxs.is_empty()
    }

    fn is_self_capture(&self, point_idx: i32) -> bool {
        let captured_type = match self.turn {
            Turn::Black => StoneType::Black,
            Turn::White => StoneType::White,
        };
        let mut search_stack = vec![point_idx];
        let mut checked_idxs = vec![];

        while let Some(i) = search_stack.pop() {
            let point = &self.board.points[i as usize];
            match point.ty {
                StoneType::Empty => return false,
                _ => {
                    if point.ty == captured_type && checked_idxs.iter().all(|&x| x != i) {
                        search_stack.extend(point.neighbors.iter());
                        checked_idxs.push(i);
                    }
                }
            }
        }
        true
    }

    fn try_select_point(&mut self, pos: Vector2<f32>) -> bool {
        let i = self.board.find_point(pos, STONE_RADIUS);
        if i >= 0 {
            let point = &mut self.board.points[i as usize];
            println!(
                "found point {:?} {:?}, neighbors {:?}",
                i, point.pos, point.neighbors
            );
            match point.ty {
                StoneType::Empty => {
                    match self.turn {
                        Turn::Black => point.ty = StoneType::Black,
                        Turn::White => point.ty = StoneType::White,
                    };
                    if !self.update_captures(i) {
                        if self.is_self_capture(i) {
                            println!("self capture");
                            let point = &mut self.board.points[i as usize];
                            point.ty = StoneType::Empty;
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            }
        } else {
            println!("no point found");
            false
        }
    }

    pub fn select_point(&mut self, pos: Vector2<f32>) {
        if self.try_select_point(pos) {
            self.turn = match self.turn {
                Turn::Black => Turn::White,
                Turn::White => Turn::Black,
            };
            self.needs_render = true;
        }
    }
}
