use crate::types::ColorIndex;
use cheers_bitboards::{BitBoard, Square};

use cheers_pregen::*;

pub fn lookup_pawn_attack(square: Square, color: ColorIndex) -> BitBoard {
    unsafe {
        *PAWN_ATTACK_TABLES
            .get_unchecked(color as usize)
            .get_unchecked(*square as usize)
    }
}

pub fn lookup_pawn_push(square: Square, color: ColorIndex) -> BitBoard {
    unsafe {
        *PAWN_PUSH_ONE_TABLES
            .get_unchecked(color as usize)
            .get_unchecked(*square as usize)
    }
}

pub fn lookup_knight(square: Square) -> BitBoard {
    unsafe { *KNIGHT_TABLE.get_unchecked(*square as usize) }
}

pub fn lookup_king(square: Square) -> BitBoard {
    unsafe { *KING_TABLE.get_unchecked(*square as usize) }
}

pub fn lookup_bishop(square: Square, blocking_mask: BitBoard) -> BitBoard {
    unsafe { *SLIDING_ATTACK_TABLE.get_unchecked(bishop_attack_index(square, blocking_mask)) }
}

pub fn lookup_rook(square: Square, blocking_mask: BitBoard) -> BitBoard {
    unsafe { *SLIDING_ATTACK_TABLE.get_unchecked(rook_attack_index(square, blocking_mask)) }
}

pub fn lookup_queen(square: Square, blocking_mask: BitBoard) -> BitBoard {
    unsafe {
        (*SLIDING_ATTACK_TABLE.get_unchecked(rook_attack_index(square, blocking_mask)))
            | (*SLIDING_ATTACK_TABLE.get_unchecked(bishop_attack_index(square, blocking_mask)))
    }
}

pub fn lookup_between(start: Square, target: Square) -> BitBoard {
    unsafe {
        *BETWEEN
            .get_unchecked(*start as usize)
            .get_unchecked(*target as usize)
    }
}

// masks to prevent A-H file wrapping
#[allow(dead_code)]
mod consts {
    use cheers_bitboards::BitBoard;

    pub const NOT_A_FILE: BitBoard = BitBoard(!0x0101010101010101);
    pub const NOT_A_B_FILES: BitBoard = BitBoard(!0x0303030303030303);
    pub const NOT_H_FILE: BitBoard = BitBoard(!0x8080808080808080);
    pub const NOT_G_H_FILES: BitBoard = BitBoard(!0xC0C0C0C0C0C0C0C0);

    // masks for ranks/files
    pub const A_FILE: BitBoard = BitBoard(0x0101010101010101);
    pub const B_FILE: BitBoard = BitBoard(0x0202020202020202);
    pub const C_FILE: BitBoard = BitBoard(0x0404040404040404);
    pub const D_FILE: BitBoard = BitBoard(0x0808080808080808);
    pub const E_FILE: BitBoard = BitBoard(0x1010101010101010);
    pub const F_FILE: BitBoard = BitBoard(0x2020202020202020);
    pub const G_FILE: BitBoard = BitBoard(0x4040404040404040);
    pub const H_FILE: BitBoard = BitBoard(0x8080808080808080);

    pub const FILES: [BitBoard; 8] = [
        A_FILE, B_FILE, C_FILE, D_FILE, E_FILE, F_FILE, G_FILE, H_FILE,
    ];

    pub const FIRST_RANK: BitBoard = BitBoard(0x00000000000000FF);
    pub const SECOND_RANK: BitBoard = BitBoard(0x000000000000FF00);
    pub const THIRD_RANK: BitBoard = BitBoard(0x0000000000FF0000);
    pub const FOURTH_RANK: BitBoard = BitBoard(0x00000000FF000000);
    pub const FIFTH_RANK: BitBoard = BitBoard(0x000000FF00000000);
    pub const SIXTH_RANK: BitBoard = BitBoard(0x0000FF0000000000);
    pub const SEVENTH_RANK: BitBoard = BitBoard(0x00FF000000000000);
    pub const EIGHTH_RANK: BitBoard = BitBoard(0xFF00000000000000);

    pub const LIGHT_SQUARES: BitBoard = BitBoard(0x5555555555555555);
    pub const DARK_SQUARES: BitBoard = BitBoard(0xAAAAAAAAAAAAAAAA);

    pub const LONG_DIAGONALS: BitBoard = BitBoard(0x8142241818244281);
}
pub use self::consts::*;

pub fn adjacent_files(file: usize) -> BitBoard {
    match file {
        0 => B_FILE,
        1 => A_FILE | C_FILE,
        2 => B_FILE | D_FILE,
        3 => C_FILE | E_FILE,
        4 => D_FILE | F_FILE,
        5 => E_FILE | G_FILE,
        6 => F_FILE | H_FILE,
        7 => G_FILE,
        _ => unreachable!(),
    }
}
