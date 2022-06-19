use std::ops::{Add, AddAssign, Sub};

use crate::bitboard::relative_board_index;

pub use self::eval_params::*;
use self::GamePhase::*;

use super::*;

pub struct EvalInfo {
    mobility_area: [BitBoard; 2],
    behind_pawns: [BitBoard; 2],
    outposts: [BitBoard; 2],
    seventh_rank: [BitBoard; 2],
}

#[derive(Copy, Clone)]
pub struct EvalScore {
    pub mg: i32,
    pub eg: i32,
}

impl EvalScore {
    pub fn zero() -> Self {
        Self { mg: 0, eg: 0 }
    }
}

impl Add<EvalScore> for EvalScore {
    type Output = Self;

    fn add(self, rhs: EvalScore) -> Self::Output {
        Self {
            mg: self.mg + rhs.mg,
            eg: self.eg + rhs.eg,
        }
    }
}

impl AddAssign<EvalScore> for EvalScore {
    fn add_assign(&mut self, rhs: EvalScore) {
        self.mg += rhs.mg;
        self.eg += rhs.eg;
    }
}

impl Sub<EvalScore> for EvalScore {
    type Output = Self;

    fn sub(self, rhs: EvalScore) -> Self::Output {
        Self {
            mg: self.mg - rhs.mg,
            eg: self.eg - rhs.eg,
        }
    }
}

pub trait TraceTarget {
    fn term(&mut self, _term: impl FnMut(&mut EvalTrace)) {}
}

impl TraceTarget for EvalTrace {
    fn term(&mut self, mut term: impl FnMut(&mut EvalTrace)) {
        term(self)
    }
}
impl TraceTarget for () {}

pub trait TracingType {
    const TRACING: bool;
}
pub struct Tracing;
pub struct NoTracing;

impl TracingType for Tracing {
    const TRACING: bool = true;
}
impl TracingType for NoTracing {
    const TRACING: bool = false;
}

pub struct EvalContext<'g, T> {
    game: &'g ChessGame,
    trace: &'g mut T,
    params: &'g EvalParams,
}

impl<'g, T: TraceTarget + Default> EvalContext<'g, T> {
    #[inline]
    pub fn evaluate(&mut self) -> i32 {
        let mut eval = EvalScore::zero();

        self.trace
            .term(|t| t.turn = self.game.current_player() as i32);

        let phase = self.game.game_phase();

        // initialise eval info
        let info = EvalInfo {
            mobility_area: [
                self.game.mobility_area(White),
                self.game.mobility_area(Black),
            ],
            behind_pawns: [
                self.game.piece_masks()[(White, Pawn)] >> 8,
                self.game.piece_masks()[(Black, Pawn)] << 8,
            ],
            outposts: [
                self.game.pawn_attack_spans(Black).inverse(),
                self.game.pawn_attacks(White).inverse(),
            ],
            seventh_rank: [SEVENTH_RANK, SECOND_RANK],
        };

        eval += self.evaluate_knights(self.game.current_player(), &info, self.params)
            - self.evaluate_knights(!self.game.current_player(), &info, self.params);

        eval += self.evaluate_bishops(self.game.current_player(), &info, self.params)
            - self.evaluate_bishops(!self.game.current_player(), &info, self.params);

        eval += self.evaluate_rooks(self.game.current_player(), &info, self.params)
            - self.evaluate_rooks(!self.game.current_player(), &info, self.params);

        eval += self.evaluate_queens(self.game.current_player(), &info, self.params)
            - self.evaluate_queens(!self.game.current_player(), &info, self.params);

        eval += self.evaluate_pawns(self.game.current_player(), &info, self.params)
            - self.evaluate_pawns(!self.game.current_player(), &info, self.params);

        eval += self.evaluate_king(self.game.current_player(), &info, self.params)
            - self.evaluate_king(!self.game.current_player(), &info, self.params);

        ((eval.mg * (256 - phase)) + (eval.eg * phase)) / 256
    }

    #[inline]
    pub fn evaluate_knights(
        &mut self,
        color: ColorIndex,
        info: &EvalInfo,
        params: &EvalParams,
    ) -> EvalScore {
        let mut eval = EvalScore::zero();

        // material value
        let count = self.game.piece_masks()[(color, Knight)].count_ones() as i32;
        eval.mg += params.piece_values[(Midgame, Knight)] * count;
        eval.eg += params.piece_values[(Endgame, Knight)] * count;
        self.trace.term(|t| t.knight_count[color as usize] = count);

        // knights behind pawns
        let knights_behind_pawns = (self.game.piece_masks()[(color, Knight)]
            & info.behind_pawns[color as usize])
            .count_ones() as i32;
        eval.mg += params.knight_behind_pawn[Midgame as usize] * knights_behind_pawns;
        eval.eg += params.knight_behind_pawn[Endgame as usize] * knights_behind_pawns;
        self.trace
            .term(|t| t.knights_behind_pawns[color as usize] = knights_behind_pawns);

        for knight in self.game.piece_masks()[(color, Knight)] {
            let relative_knight = relative_board_index(knight, color);
            // placement
            eval.mg += params.piece_tables[(Midgame, Knight, relative_knight as u8)];
            eval.eg += params.piece_tables[(Endgame, Knight, relative_knight as u8)];
            self.trace
                .term(|t| t.knight_placement[relative_knight as usize][color as usize] += 1);

            // king distance
            let king = self.game.piece_masks()[(color, King)].lsb_index() as i32;
            let distance = (king % 8 - knight as i32 % 8)
                .abs()
                .max((king / 8 - knight as i32 / 8).abs());
            if distance >= 4 {
                eval.mg += params.knight_king_distance[distance as usize - 4][Midgame as usize];
                eval.eg += params.knight_king_distance[distance as usize - 4][Endgame as usize];
                self.trace
                    .term(|t| t.knight_king_distance[distance as usize - 4][color as usize] += 1);
            }

            // outposts
            if (BitBoard(1 << knight) & info.outposts[color as usize]).is_not_empty() {
                let defended = (lookup_pawn_attack(knight as usize, !color)
                    & self.game.piece_masks()[(color, Pawn)])
                    .is_not_empty();
                eval.mg += params.knight_outpost[defended as usize][Midgame as usize];
                eval.eg += params.knight_outpost[defended as usize][Endgame as usize];
                self.trace
                    .term(|t| t.knight_outposts[defended as usize][color as usize] += 1);
            }

            // mobility
            let attacks = lookup_knight(knight.into());
            let mobility = (attacks & info.mobility_area[color as usize]).count_ones() as usize;
            eval.mg += params.knight_mobility[mobility][Midgame as usize];
            eval.eg += params.knight_mobility[mobility][Endgame as usize];
            self.trace
                .term(|t| t.knight_mobility[mobility][color as usize] += 1);
        }
        eval
    }

    #[inline]
    pub fn evaluate_bishops(
        &mut self,
        color: ColorIndex,
        info: &EvalInfo,
        params: &EvalParams,
    ) -> EvalScore {
        let mut eval = EvalScore::zero();

        // material value
        let count = self.game.piece_masks()[(color, Bishop)].count_ones() as i32;
        eval.mg += params.piece_values[(Midgame, Bishop)] * count;
        eval.eg += params.piece_values[(Endgame, Bishop)] * count;
        self.trace.term(|t| t.bishop_count[color as usize] = count);

        // bishops behind pawns
        let bishops_behind_pawns = (self.game.piece_masks()[(color, Bishop)]
            & info.behind_pawns[color as usize])
            .count_ones() as i32;
        eval.mg += params.bishop_behind_pawn[Midgame as usize] * bishops_behind_pawns;
        eval.eg += params.bishop_behind_pawn[Endgame as usize] * bishops_behind_pawns;
        self.trace
            .term(|t| t.bishops_behind_pawns[color as usize] = bishops_behind_pawns);

        // bishop pair
        if (self.game.piece_masks()[(color, Bishop)] & LIGHT_SQUARES).count_ones() >= 1
            && (self.game.piece_masks()[(color, Bishop)] & DARK_SQUARES).count_ones() >= 1
        {
            eval.mg += params.bishop_pair[Midgame as usize];
            eval.eg += params.bishop_pair[Endgame as usize];
            self.trace.term(|t| t.bishop_pair[color as usize] += 1);
        }

        // long diagonals
        let bishop_long_diagonals =
            (self.game.piece_masks()[(color, Bishop)] & LONG_DIAGONALS).count_ones() as i32;
        eval.mg += params.bishop_long_diagonal[Midgame as usize] * bishop_long_diagonals;
        eval.eg += params.bishop_long_diagonal[Endgame as usize] * bishop_long_diagonals;
        self.trace
            .term(|t| t.bishop_long_diagonals[color as usize] = bishop_long_diagonals);

        for bishop in self.game.piece_masks()[(color, Bishop)] {
            // placement
            let relative_bishop = relative_board_index(bishop, color);
            eval.mg += params.piece_tables[(Midgame, Bishop, relative_bishop as u8)];
            eval.eg += params.piece_tables[(Endgame, Bishop, relative_bishop as u8)];
            self.trace
                .term(|t| t.bishop_placement[relative_bishop as usize][color as usize] += 1);

            // king distance
            let king = self.game.piece_masks()[(color, King)].lsb_index() as i32;
            let distance = (king % 8 - bishop as i32 % 8)
                .abs()
                .max((king / 8 - bishop as i32 / 8).abs());
            if distance >= 4 {
                eval.mg += params.bishop_king_distance[distance as usize - 4][Midgame as usize];
                eval.eg += params.bishop_king_distance[distance as usize - 4][Endgame as usize];
                self.trace
                    .term(|t| t.bishop_king_distance[distance as usize - 4][color as usize] += 1);
            }

            // outposts
            if (BitBoard(1 << bishop) & info.outposts[color as usize]).is_not_empty() {
                let defended = (lookup_pawn_attack(bishop as usize, !color)
                    & self.game.piece_masks()[(color, Pawn)])
                    .is_not_empty();
                eval.mg += params.bishop_outpost[defended as usize][Midgame as usize];
                eval.eg += params.bishop_outpost[defended as usize][Endgame as usize];
                self.trace
                    .term(|t| t.bishop_outposts[defended as usize][color as usize] += 1);
            }

            // mobility
            let attacks = lookup_bishop(bishop as usize, self.game.combined());
            let mobility = (attacks & info.mobility_area[color as usize]).count_ones() as usize;
            eval.mg += params.bishop_mobility[mobility][Midgame as usize];
            eval.eg += params.bishop_mobility[mobility][Endgame as usize];
            self.trace
                .term(|t| t.bishop_mobility[mobility][color as usize] += 1);
        }
        eval
    }

    #[inline]
    pub fn evaluate_rooks(
        &mut self,
        color: ColorIndex,
        info: &EvalInfo,
        params: &EvalParams,
    ) -> EvalScore {
        let mut eval = EvalScore::zero();

        // material value
        let count = self.game.piece_masks()[(color, Rook)].count_ones() as i32;
        eval.mg += params.piece_values[(Midgame, Rook)] * count;
        eval.eg += params.piece_values[(Endgame, Rook)] * count;
        self.trace.term(|t| t.rook_count[color as usize] = count);

        // rooks on seventh
        let rooks_on_seventh = (self.game.piece_masks()[(color, Rook)]
            & info.seventh_rank[color as usize])
            .count_ones() as i32;
        eval.mg += params.rook_on_seventh[Midgame as usize] * rooks_on_seventh;
        eval.mg += params.rook_on_seventh[Endgame as usize] * rooks_on_seventh;
        self.trace
            .term(|t| t.rooks_on_seventh[color as usize] = rooks_on_seventh);

        for rook in self.game.piece_masks()[(color, Rook)] {
            // placement
            let relative_rook = relative_board_index(rook, color);
            eval.mg += params.piece_tables[(Midgame, Rook, relative_rook as u8)];
            eval.eg += params.piece_tables[(Endgame, Rook, relative_rook as u8)];
            self.trace
                .term(|t| t.rook_placement[relative_rook as usize][color as usize] += 1);

            // open files
            if (self.game.piece_masks()[(color, Pawn)] & FILES[rook as usize % 8]).is_empty() {
                let open = (self.game.piece_masks()[(!color, Pawn)] & FILES[rook as usize % 8])
                    .is_empty() as usize;
                eval.mg += params.rook_open_file[open][Midgame as usize];
                eval.eg += params.rook_open_file[open][Endgame as usize];
                self.trace
                    .term(|t| t.rook_open_files[open][color as usize] += 1);
            }

            // mobility
            let attacks = lookup_rook(rook as usize, self.game.combined());
            let mobility = (attacks & info.mobility_area[color as usize]).count_ones() as usize;
            eval.mg += params.rook_mobility[mobility][Midgame as usize];
            eval.eg += params.rook_mobility[mobility][Endgame as usize];
            self.trace
                .term(|t| t.rook_mobility[mobility][color as usize] += 1);
        }
        eval
    }

    #[inline]
    pub fn evaluate_queens(
        &mut self,
        color: ColorIndex,
        info: &EvalInfo,
        params: &EvalParams,
    ) -> EvalScore {
        let mut eval = EvalScore::zero();

        // material value
        let count = self.game.piece_masks()[(color, Queen)].count_ones() as i32;
        eval.mg += params.piece_values[(Midgame, Queen)] * count;
        eval.eg += params.piece_values[(Endgame, Queen)] * count;
        self.trace.term(|t| t.queen_count[color as usize] = count);

        for queen in self.game.piece_masks()[(color, Queen)] {
            // placement
            let relative_queen = relative_board_index(queen, color);
            eval.mg += params.piece_tables[(Midgame, Queen, relative_queen as u8)];
            eval.eg += params.piece_tables[(Endgame, Queen, relative_queen as u8)];
            self.trace
                .term(|t| t.queen_placement[relative_queen as usize][color as usize] += 1);

            // discovery risk
            if self
                .game
                .discovered_attacks(queen as usize, color)
                .is_not_empty()
            {
                eval.mg += params.queen_discovery_risk[Midgame as usize];
                eval.eg += params.queen_discovery_risk[Endgame as usize];
                self.trace.term(|t| t.queen_discovery_risks[color as usize] += 1);
            }

            // mobility
            let attacks = lookup_queen(queen as usize, self.game.combined());
            let mobility = (attacks & info.mobility_area[color as usize]).count_ones() as usize;
            eval.mg += params.queen_mobility[mobility][Midgame as usize];
            eval.eg += params.queen_mobility[mobility][Endgame as usize];
            self.trace.term(|t| t.queen_mobility[mobility as usize][color as usize] += 1);
        }
        eval
    }

    #[inline]
    pub fn evaluate_king(
        &mut self,
        color: ColorIndex,
        _info: &EvalInfo,
        params: &EvalParams,
    ) -> EvalScore {
        let mut eval = EvalScore::zero();

        // placement
        let king = relative_board_index(self.game.piece_masks()[(color, King)].lsb_index() as u8, color);
        eval.mg += params.piece_tables[(Midgame, King, king as u8)];
        eval.eg += params.piece_tables[(Endgame, King, king as u8)];
        self.trace.term(|t| t.king_placement[king as usize][color as usize] += 1);

        eval
    }

    #[inline]
    pub fn evaluate_pawns(
        &mut self,
        color: ColorIndex,
        _info: &EvalInfo,
        params: &EvalParams,
    ) -> EvalScore {
        let mut eval = EvalScore::zero();

        // material value
        let count = self.game.piece_masks()[(color, Pawn)].count_ones() as i32;
        eval.mg += params.piece_values[(Midgame, Pawn)] * count;
        eval.eg += params.piece_values[(Endgame, Pawn)] * count;
        self.trace.term(|t| t.pawn_count[color as usize] = count);

        // passed pawns
        let front_spans = self.game.pawn_front_spans(!color);
        let all_front_spans =
            front_spans | (front_spans & NOT_H_FILE) << 1 | (front_spans & NOT_A_FILE) >> 1;
        let passers =
            (self.game.piece_masks()[(color, Pawn)] & all_front_spans.inverse()).count_ones() as i32;
        eval.mg += params.passed_pawn[Midgame as usize] * passers;
        eval.eg += params.passed_pawn[Endgame as usize] * passers;
        self.trace.term(|t| t.passed_pawns[color as usize] = passers);

        // unsupported double pawns
        let pawns = self.game.piece_masks()[(color, Pawn)];
        let shifted = if color == White {
            pawns >> 8
        } else {
            pawns << 8
        };
        let double_pawns =
            (pawns & shifted & ((pawns & NOT_H_FILE) << 1 | (pawns & NOT_A_FILE >> 1)).inverse())
                .count_ones() as i32;
        eval.mg += params.double_pawn[Midgame as usize] * double_pawns;
        eval.eg += params.double_pawn[Endgame as usize] * double_pawns;
        self.trace.term(|t| t.double_pawns[color as usize] = double_pawns);

        for pawn in self.game.piece_masks()[(color, Pawn)] {
            // placement
            let relative_pawn = relative_board_index(pawn, color);
            eval.mg += params.piece_tables[(Midgame, Pawn, relative_pawn as u8)];
            eval.eg += params.piece_tables[(Endgame, Pawn, relative_pawn as u8)];
            self.trace.term(|t| t.pawn_placement[relative_pawn as usize][color as usize] += 1);

            let board = BitBoard(1 << pawn);
            let attacks = match color {
                White => ((board & NOT_H_FILE) << 9) | ((board & NOT_A_FILE) << 7),
                Black => ((board & NOT_A_FILE) >> 9) | ((board & NOT_H_FILE) >> 7),
            };
            let neighbors = self.game.piece_masks()[(color, Pawn)] & adjacent_files(pawn as usize % 8);

            // isolated pawns
            if attacks.is_empty() && neighbors.is_empty() {
                eval.mg += params.isolated_pawn[pawn as usize % 8][Midgame as usize];
                eval.eg += params.isolated_pawn[pawn as usize % 8][Endgame as usize];
                self.trace.term(|t| t.isolated_pawns[pawn as usize % 8][color as usize] += 1);
            }
        }

        eval
    }
}

impl ChessGame {
    #[inline]
    pub fn mobility_area(&self, color: ColorIndex) -> BitBoard {
        let blocked_pawns = match color {
            White => self.piece_masks[(White, Pawn)] & (self.piece_masks[(Black, Pawn)] >> 8),
            Black => self.piece_masks[(Black, Pawn)] & (self.piece_masks[(White, Pawn)] << 8),
        };

        // exclude squares attacked by enemy pawns, our blocked pawns and our king
        (self.pawn_attacks(!color) | blocked_pawns | self.piece_masks[(color, King)]).inverse()
    }

    #[inline]
    pub fn evaluate<T: TraceTarget + Default>(&self) -> (i32, T) {
        let mut trace = T::default();
        let mut eval = EvalContext {
            game: self,
            trace: &mut trace,
            params: &EVAL_PARAMS,
        };
        let score = eval.evaluate();
        (score, trace)
    }

    #[inline]
    pub fn game_phase(&self) -> i32 {
        let knight_phase = 1;
        let bishop_phase = 1;
        let rook_phase = 2;
        let queen_phase = 4;

        let total_phase = knight_phase * 4 + bishop_phase * 4 + rook_phase * 4 + queen_phase * 2;

        let mut phase: i32 = 0;

        phase += (self.piece_masks[(White, Knight)] | self.piece_masks[(Black, Knight)])
            .count_ones() as i32
            * knight_phase;
        phase += (self.piece_masks[(White, Bishop)] | self.piece_masks[(Black, Bishop)])
            .count_ones() as i32
            * bishop_phase;
        phase += (self.piece_masks[(White, Rook)] | self.piece_masks[(Black, Rook)]).count_ones()
            as i32
            * rook_phase;
        phase += (self.piece_masks[(White, Queen)] | self.piece_masks[(Black, Queen)]).count_ones()
            as i32
            * queen_phase;

        (256 * (total_phase - phase)) / total_phase
    }
}