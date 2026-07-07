//! Chunked tile storage for infinite levels.
//!
//! A level layer is an unbounded plane of `CHUNK_SIZE`² tile chunks, generated on demand
//! around the player and saved/unloaded when far away. Tile coordinates are global i32;
//! chunk coordinates are `tile >> CHUNK_SHIFT`.

use std::collections::HashMap;

pub const CHUNK_SHIFT: i32 = 6;
/// 64x64 tiles per chunk.
pub const CHUNK_SIZE: i32 = 1 << CHUNK_SHIFT;
const CHUNK_AREA: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;

/// How many chunks around the player stay loaded (radius, in chunks).
pub const LOAD_RADIUS: i32 = 2;
/// Chunks farther than this get saved + unloaded.
pub const UNLOAD_RADIUS: i32 = 4;

#[derive(Clone)]
pub struct Chunk {
    pub tiles: Vec<u8>,
    pub data: Vec<u8>,
    /// Map fog-of-war: explored tiles (the old per-level `visible` array).
    pub visible: Vec<bool>,
    /// Needs saving before unload.
    pub dirty: bool,
}

impl Chunk {
    pub fn new() -> Chunk {
        Chunk {
            tiles: vec![0; CHUNK_AREA],
            data: vec![0; CHUNK_AREA],
            visible: vec![false; CHUNK_AREA],
            dirty: false,
        }
    }

    #[inline]
    fn idx(local_x: i32, local_y: i32) -> usize {
        (local_x + local_y * CHUNK_SIZE) as usize
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Chunk::new()
    }
}

/// The chunk map of one infinite level layer.
#[derive(Default)]
pub struct ChunkMap {
    chunks: HashMap<(i32, i32), Chunk>,
}

#[inline]
pub fn chunk_coord(tile: i32) -> i32 {
    tile >> CHUNK_SHIFT
}

#[inline]
fn local(tile: i32) -> i32 {
    tile & (CHUNK_SIZE - 1)
}

impl ChunkMap {
    pub fn is_loaded(&self, cx: i32, cy: i32) -> bool {
        self.chunks.contains_key(&(cx, cy))
    }

    pub fn insert(&mut self, cx: i32, cy: i32, chunk: Chunk) {
        self.chunks.insert((cx, cy), chunk);
    }

    pub fn remove(&mut self, cx: i32, cy: i32) -> Option<Chunk> {
        self.chunks.remove(&(cx, cy))
    }

    pub fn get(&self, cx: i32, cy: i32) -> Option<&Chunk> {
        self.chunks.get(&(cx, cy))
    }

    pub fn loaded_coords(&self) -> Vec<(i32, i32)> {
        self.chunks.keys().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Tile id at a global tile coordinate; None when the chunk isn't loaded.
    pub fn tile(&self, x: i32, y: i32) -> Option<u8> {
        self.chunks
            .get(&(chunk_coord(x), chunk_coord(y)))
            .map(|c| c.tiles[Chunk::idx(local(x), local(y))])
    }

    pub fn data(&self, x: i32, y: i32) -> Option<u8> {
        self.chunks
            .get(&(chunk_coord(x), chunk_coord(y)))
            .map(|c| c.data[Chunk::idx(local(x), local(y))])
    }

    pub fn set_tile(&mut self, x: i32, y: i32, id: u8, data: u8) {
        if let Some(c) = self.chunks.get_mut(&(chunk_coord(x), chunk_coord(y))) {
            let i = Chunk::idx(local(x), local(y));
            c.tiles[i] = id;
            c.data[i] = data;
            c.dirty = true;
        }
    }

    pub fn set_data(&mut self, x: i32, y: i32, data: u8) {
        if let Some(c) = self.chunks.get_mut(&(chunk_coord(x), chunk_coord(y))) {
            c.data[Chunk::idx(local(x), local(y))] = data;
            c.dirty = true;
        }
    }

    pub fn is_visible(&self, x: i32, y: i32) -> bool {
        self.chunks
            .get(&(chunk_coord(x), chunk_coord(y)))
            .map(|c| c.visible[Chunk::idx(local(x), local(y))])
            .unwrap_or(false)
    }

    pub fn mark_visible(&mut self, x: i32, y: i32) {
        if let Some(c) = self.chunks.get_mut(&(chunk_coord(x), chunk_coord(y))) {
            let i = Chunk::idx(local(x), local(y));
            if !c.visible[i] {
                c.visible[i] = true;
                c.dirty = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coords_round_trip() {
        let mut m = ChunkMap::default();
        m.insert(-1, -1, Chunk::new());
        m.insert(0, 0, Chunk::new());
        m.set_tile(-1, -1, 7, 3);
        m.set_tile(0, 0, 9, 0);
        m.set_tile(63, 63, 5, 1);
        assert_eq!(m.tile(-1, -1), Some(7));
        assert_eq!(m.data(-1, -1), Some(3));
        assert_eq!(m.tile(0, 0), Some(9));
        assert_eq!(m.tile(63, 63), Some(5));
        assert_eq!(m.tile(64, 0), None); // unloaded chunk
        assert_eq!(m.tile(-65, 0), None);
    }
}
