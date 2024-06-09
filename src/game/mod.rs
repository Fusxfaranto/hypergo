use std::{f64::consts::PI, marker::PhantomData, ptr};

use cgmath::{abs_diff_eq, relative_eq, MetricSpace, Vector2, Zero};

pub mod render;
use render::*;

use crate::geometry::*;

/* struct PanicIterator<T> {
    phantom: PhantomData<T>,
}

impl<T> PanicIterator<T> {
    fn new() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<T> Iterator for PanicIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
} */

pub const MAX_STONES: u64 = 1024 * 16;
pub const STONE_RADIUS: f32 = 0.4;

#[derive(PartialEq)]
enum StoneType {
    Empty,
    Black,
    White,
}

struct BoardPoint<SpinorT: Spinor> {
    pos: SpinorT::Point,
    transform: SpinorT,
    neighbors: Vec<i32>,
    ty: StoneType,
    reversed: bool,
}

struct Board<SpinorT: Spinor> {
    points: Vec<BoardPoint<SpinorT>>,
    links: Vec<(i32, i32)>,
    tiling_parameters: TilingParameters,
    //neighbor_directions: Vec<SpinorT>,
}

impl<SpinorT: Spinor> Board<SpinorT> {
    fn make_board(tiling_parameters: TilingParameters, edge_len: usize) -> Self {
        // TODO support even size probably?
        assert!(edge_len % 2 == 1);

        let neighbor_directions: Vec<SpinorT> = (0..tiling_parameters.around_vertex)
            .map(|i| {
                SpinorT::translation(
                    tiling_parameters.distance,
                    i as f64 * tiling_parameters.angle,
                )
            })
            .collect();

        let mut board = Self {
            points: Vec::new(),
            links: Vec::new(),
            tiling_parameters,
            //neighbor_directions: neighbor_directions.clone(),
        };

        let mut test_count = 1;

        let reverse_neighbor_directions: Vec<SpinorT> =
            neighbor_directions.iter().map(|d| d.reverse()).collect();

        board.add_point(
            &neighbor_directions,
            &reverse_neighbor_directions,
            SpinorT::one(),
            false,
        );
        let mut start_i = 0;
        for ring in 1..(edge_len / 2 + 1) {
            let l = board.points.len();
            for i in start_i..l {
                for j in 0..neighbor_directions.len() {
                    let mut cur_transform = board.points[i].transform;
                    //for &dir in (&neighbor_directions[0..1]).iter().cycle().skip(j) {
                    for (k, &dir) in neighbor_directions.iter().cycle().skip(j).enumerate() {
                        let link_reversed = (k % 2 == 1) ^ board.points[i].reversed;
                        cur_transform = if link_reversed {
                            cur_transform * dir.reverse()
                        } else {
                            cur_transform * dir
                        };
                        let pos = cur_transform.apply(SpinorT::Point::zero());
                        if board.find_point(pos, 1e-3) != -1 {
                            if k == 0 {
                                continue;
                            } else {
                                break;
                            }
                        }
                        board.add_point(
                            &neighbor_directions,
                            &reverse_neighbor_directions,
                            cur_transform,
                            !link_reversed,
                        );

                        test_count += 1;
                        if test_count >= 2500 {
                            return board;
                        }
                    }
                }
            }
            start_i = l;
        }

        board
    }

    // TODO shouldn't need to check (non-)reverse neighbors on every point, be smarter
    fn add_point(
        &mut self,
        neighbor_directions: &Vec<SpinorT>,
        reverse_neighbor_directions: &Vec<SpinorT>,
        transform: SpinorT,
        reversed: bool,
    ) {
        let mut point = BoardPoint {
            pos: transform.apply(SpinorT::Point::zero()),
            transform,
            neighbors: Vec::new(),
            ty: StoneType::Empty,
            reversed,
        };
        let this_idx = self.points.len() as i32;

        println!("adding point {:?} at {:?}", self.points.len(), point.pos);
        println!("with transform {:?}", transform);
        // not the best approach
        for dir in neighbor_directions
            .iter()
            .chain(reverse_neighbor_directions.iter())
        {
            //let i = self.find_point(dir.apply(point.pos), 0.1);
            let checking_pos = (point.transform * *dir).apply(SpinorT::Point::zero());
            println!("checking for neighbor at  {:?}", checking_pos);
            let i = self.find_point(checking_pos, 0.1);
            if i >= 0 {
                point.neighbors.push(i);
                self.points[i as usize].neighbors.push(this_idx);
                self.links.push((i, this_idx));
                println!("adding link {:?}", self.links.last().unwrap());
            }
        }
        self.points.push(point);
    }

    // TODO use some kind of spatial data structure for this?
    fn find_point(&self, pos: SpinorT::Point, dist: f64) -> i32 {
        for (i, point) in self.points.iter().enumerate() {
            if pos.distance(point.pos) <= dist {
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

pub struct GameState<SpinorT: Spinor> {
    board: Board<SpinorT>,
    turn: Turn,
    pub needs_render: bool,
}

impl<SpinorT: Spinor> GameState<SpinorT> {
    pub fn new() -> Self {
        let board = if cfg!(feature = "euclidian_geometry") {
            Board::make_board(TilingParameters::new::<SpinorT>(6, 3), 15)
        } else {
            Board::make_board(TilingParameters::new::<SpinorT>(10, 4), 7)
        };
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

    fn try_select_point(&mut self, pos: SpinorT::Point) -> bool {
        let i = self.board.find_point(pos, STONE_RADIUS as f64);
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
            println!("no point found at {:?}", pos);
            false
        }
    }

    pub fn select_point(&mut self, pos: SpinorT::Point) {
        if self.try_select_point(pos) {
            self.turn = match self.turn {
                Turn::Black => Turn::White,
                Turn::White => Turn::Black,
            };
            self.needs_render = true;
        }
    }
}
