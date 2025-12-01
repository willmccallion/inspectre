pub trait Prefetcher {
    /// Returns a list of memory addresses to fetch into the cache immediately.
    fn observe(&mut self, addr: u64, hit: bool) -> Vec<u64>;
}

pub use self::next_line::NextLinePrefetcher;
pub use self::stride::StridePrefetcher;

pub mod next_line;
pub mod stride;
