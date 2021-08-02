use crate::{
    bitboard::BitBoards,
    lookup_tables::*,
    piece_tables::*,
    types::{ColorIndex, PieceIndex},
    utils::flip_square,
};
use ColorIndex::*;
use PieceIndex::*;

pub struct PieceValues([i32; 5]);
impl std::ops::Index<PieceIndex> for PieceValues {
    type Output = i32;

    fn index(&self, index: PieceIndex) -> &Self::Output {
        &self.0[index as usize]
    }
}

pub mod consts {
    use crate::evaluate::PieceValues;

    // piece values shamelessly stolen from stockfish midgame numbers
    pub const PIECE_VALUES: PieceValues = PieceValues([
        198,  // pawn
        817,  // knight
        836,  // bishop
        1270, // rook
        2521, // queen
    ]);

    pub const CHECKMATE_SCORE: i32 = -10000;
    pub const ILLEGAL_MOVE_SCORE: i32 = 100000;
    pub const DRAW_SCORE: i32 = 0;
}
use consts::*;

impl BitBoards {
    /// Static evaluation of the board position, positive value for player advantage, negative for opponent advantage
    pub fn evaluate(&self, color: ColorIndex) -> i32 {
        let phase = self.game_phase();
        ((self.phase_eval(color, GamePhase::MidGame) * (256 - phase))
            + (self.phase_eval(color, GamePhase::EndGame) * phase))
            / 256
    }
    
    fn phase_eval(&self, color: ColorIndex, phase: GamePhase) -> i32 {
        let mut result = 0;

        result += self.material_count(color) - self.material_count(!color);

        if self.insufficient_mating_material(result) {
            return DRAW_SCORE;
        }
        let placement = self.piece_placement(phase);

        result += placement[color as usize] - placement[!color as usize];

        result += self.pawn_shield_score(color, phase) - self.pawn_shield_score(!color, phase);

        result
    }

    /// returns the phase of the game between midgame and endgame for tapered eval
    fn game_phase(&self) -> i32 {
        let knight_phase = 1;
        let bishop_phase = 1;
        let rook_phase = 2;
        let queen_phase = 4;

        let total_phase = 4 * knight_phase + 4 * bishop_phase + 4 * rook_phase + 4 * queen_phase;

        let mut phase = total_phase;

        phase -= knight_phase * (self.piece_masks[Knight].count_ones() as i32);
        phase -= bishop_phase * (self.piece_masks[Bishop].count_ones() as i32);
        phase -= rook_phase * (self.piece_masks[Rook].count_ones() as i32);
        phase -= queen_phase * (self.piece_masks[Queen].count_ones() as i32);

        (phase * 256 + (total_phase / 2)) / total_phase
    }

    //TODO: endgame piece values
    fn material_count(&self, color: ColorIndex) -> i32 {
        let mut result = 0;

        result += (self.piece_masks[Pawn] & self.color_masks[color]).count_ones() as i32
            * PIECE_VALUES[Pawn];
        result += (self.piece_masks[Knight] & self.color_masks[color]).count_ones() as i32
            * PIECE_VALUES[Knight];
        result += (self.piece_masks[Bishop] & self.color_masks[color]).count_ones() as i32
            * PIECE_VALUES[Bishop];
        result += (self.piece_masks[Rook] & self.color_masks[color]).count_ones() as i32
            * PIECE_VALUES[Rook];
        result += (self.piece_masks[Queen] & self.color_masks[color]).count_ones() as i32
            * PIECE_VALUES[Queen];

        result
    }

    fn piece_placement(&self, phase: GamePhase) -> [i32; 2] {
        let mut result = [0, 0];
        self.piece_list
            .iter()
            .enumerate()
            .for_each(|(square, piece_opt)| {
                if let Some((piece, color)) = piece_opt {
                    result[*color as usize] += PIECE_TABLES[(phase, *piece)][if *color == White {
                        square
                    } else {
                        flip_square(square)
                    }]
                }
            });

        result
    }

    /// Assumes if a side has no pawns and <4 material advantage that it is drawn
    fn insufficient_mating_material(&self, material_balance: i32) -> bool {
        (self.piece_masks[Pawn] & self.color_masks[self.current_player] == 0
            && material_balance < 4 * PIECE_VALUES[Pawn])
            || (self.piece_masks[Pawn] & self.color_masks[!self.current_player] == 0
                && material_balance > -4 * PIECE_VALUES[Pawn])
    }

    /// if the king moves away from the center on the back rank (castling), add score for having pawns nearby
    fn pawn_shield_score(&self, color: ColorIndex, phase: GamePhase) -> i32 {
        if phase == GamePhase::EndGame {
            return 0;
        }

        let mut result = 0;
        let file = (self.piece_masks[King] & self.color_masks[color]).trailing_zeros() % 8;
        let rank = (self.piece_masks[King] & self.color_masks[color]).trailing_zeros() / 8;
        if file > 4 && rank - (7 * color as u32) == 0 {
            // kingside
            result += ((self.piece_masks[Pawn] & self.color_masks[color])
                & (SEVENTH_RANK * color as u64 | SECOND_RANK * (1 - color as u64))
                & (F_FILE | G_FILE | H_FILE))
                .count_ones() as i32
                * (PIECE_VALUES[Pawn] / 2);
            result += ((self.piece_masks[Pawn] & self.color_masks[color])
                & (SIXTH_RANK * color as u64 | THIRD_RANK * (1 - color as u64))
                & (F_FILE | G_FILE | H_FILE))
                .count_ones() as i32
                * (PIECE_VALUES[Pawn] / 3);
        } else if file < 3 && rank - (7 * color as u32) == 0 {
            // queenside
            result += ((self.piece_masks[Pawn] & self.color_masks[color])
                & (SEVENTH_RANK * color as u64 | SECOND_RANK * (1 - color as u64))
                & (A_FILE | B_FILE | C_FILE))
                .count_ones() as i32
                * (PIECE_VALUES[Pawn] / 2);
            result += ((self.piece_masks[Pawn] & self.color_masks[color])
                & (SIXTH_RANK * color as u64 | THIRD_RANK * (1 - color as u64))
                & (A_FILE | B_FILE | C_FILE))
                .count_ones() as i32
                * (PIECE_VALUES[Pawn] / 3);
        }
        result
    }
}
