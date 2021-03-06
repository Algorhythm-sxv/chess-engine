use std::sync::{atomic::*, Arc, RwLock};

use cheers_bitboards::Square;

use crate::{moves::Move, types::PieceIndex};

pub const TT_DEFAULT_SIZE: usize = 1 << 22; // 2^22 entries for ~64MB

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Exact,
    UpperBound,
    LowerBound,
}

impl NodeType {
    pub fn from_u8(n: u8) -> Self {
        match n {
            0 => NodeType::Exact,
            1 => NodeType::UpperBound,
            2 => NodeType::LowerBound,
            _ => unreachable!(),
        }
    }
}

pub struct TTEntry {
    pub score: i32,
    pub depth: i8,
    pub move_start: Square,
    pub move_target: Square,
    pub promotion: PieceIndex,
    pub node_type: NodeType,
    pub double_pawn_push: bool,
    pub en_passent_capture: bool,
    pub castling: bool,
}

impl TTEntry {
    pub fn from_data(data: u64) -> Self {
        Self {
            score: (data & 0xFFFFFFFF) as i32,
            depth: ((data >> 32) & 0xFF) as i8,
            move_start: ((data >> (32 + 8)) & 0xFF).into(),
            move_target: ((data >> (32 + 8 + 8)) & 0xFF).into(),
            promotion: PieceIndex::from_u8(((data >> (32 + 8 + 8 + 8)) & 0b111) as u8),
            node_type: NodeType::from_u8(((data >> (32 + 8 + 8 + 8 + 3)) & 0b11) as u8),
            double_pawn_push: ((data >> (32 + 8 + 8 + 8 + 3 + 2)) & 0b1) != 0,
            en_passent_capture: ((data >> (32 + 8 + 8 + 8 + 3 + 2 + 1)) & 0b1) != 0,
            castling: ((data >> (32 + 8 + 8 + 8 + 3 + 2 + 1 + 1)) & 0b1) != 0,
        }
    }
}

#[derive(Default)]
struct Entry {
    key: AtomicU64,
    data: AtomicU64,
}

#[derive(Clone)]
pub struct TranspositionTable {
    table: Arc<RwLock<Vec<Entry>>>,
}

impl TranspositionTable {
    pub fn new(table_size_mb: usize) -> Self {
        let mut length = table_size_mb * 1024 * 1024 / std::mem::size_of::<Entry>();
        if length != 0 {
            length = length.next_power_of_two();
        }
        let mut table = Vec::with_capacity(length);
        for _ in 0..length {
            table.push(Entry::default());
        }
        Self {
            table: Arc::new(RwLock::new(table)),
        }
    }

    pub fn set_size(&mut self, size_mb: usize) {
        let mut length = size_mb * 1024 * 1024 / std::mem::size_of::<Entry>();
        length = length.next_power_of_two();
        self.table
            .write()
            .unwrap()
            .resize_with(length, Entry::default);
    }

    pub fn set(&self, hash: u64, best_move: Move, depth: i8, score: i32, node_type: NodeType) {
        use self::Ordering::*;
        let table = self.table.read().unwrap();
        let index = hash as usize & (table.len() - 1);

        let stored = match table.get(index) {
            Some(entry) => entry,
            None => return,
        };

        let stored_depth = (stored.data.load(Acquire) >> 24) & 0xFF;
        if stored_depth > depth as u64 {
            // depth-preferred replacement
            return;
        }

        let mut data = 0u64;
        data |= score as u32 as u64;
        data |= ((depth as u8) as u64) << 32;
        data |= (*best_move.start() as u64) << (32 + 8);
        data |= (*best_move.target() as u64) << (32 + 8 + 8);
        data |= (best_move.promotion() as u64) << (32 + 8 + 8 + 8);
        data |= (node_type as u64) << (32 + 8 + 8 + 8 + 3);
        data |= (best_move.double_pawn_push() as u64) << (32 + 8 + 8 + 8 + 3 + 2);
        data |= (best_move.en_passent() as u64) << (32 + 8 + 8 + 8 + 3 + 2 + 1);
        data |= (best_move.castling() as u64) << (32 + 8 + 8 + 8 + 3 + 2 + 1 + 1);

        stored.key.store(hash ^ data, Release);
        stored.data.store(data, Release);
    }

    pub fn get(&self, hash: u64) -> Option<TTEntry> {
        use self::Ordering::*;
        let table = self.table.read().unwrap();
        let index = hash as usize & (table.len() - 1);

        let stored = table.get(index)?;

        let data = stored.data.load(Acquire);

        if stored.key.load(Acquire) ^ data == hash {
            // entry is valid, return data
            Some(TTEntry::from_data(data))
        } else {
            // key and data didn't match, invalid entry
            None
        }
    }
}
