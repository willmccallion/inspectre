pub trait ReplacementPolicy {
    fn update(&mut self, set: usize, way: usize);
    fn get_victim(&mut self, set: usize) -> usize;
}

pub use self::fifo::FifoPolicy;
pub use self::lru::LruPolicy;
pub use self::plru::PlruPolicy;
pub use self::random::RandomPolicy;

mod fifo;
mod lru;
mod plru;
mod random;
