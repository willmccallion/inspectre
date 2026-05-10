//! Hierarchical, path-addressed statistics.
//!
//! Components write to paths like `system.cpu0.cache.l1d.hits` or
//! `system.memctrl.ch0.bank3.row_hits`. The tree shape lets multi-core and
//! multi-channel configurations stay readable, supports phase-based reset
//! (warm up, reset, measure), and dumps to text / JSON / CSV.

use std::collections::BTreeMap;
use std::io::{self, Write};

/// A scalar counter.
#[derive(Clone, Copy, Debug, Default)]
pub struct Counter(u64);

impl Counter {
    /// Increments the counter by one.
    #[inline]
    pub const fn inc(&mut self) {
        self.0 = self.0.saturating_add(1);
    }

    /// Adds `n` to the counter.
    #[inline]
    pub const fn add(&mut self, n: u64) {
        self.0 = self.0.saturating_add(n);
    }

    /// Returns the current value.
    #[inline]
    pub const fn get(&self) -> u64 {
        self.0
    }

    /// Resets the counter to zero.
    #[inline]
    pub const fn reset(&mut self) {
        self.0 = 0;
    }
}

/// A power-of-two-bucketed histogram for latency / queue-depth distributions.
///
/// Bucket `i` counts samples in `[2^i, 2^(i+1))`. Bucket 0 counts zeroes too.
#[derive(Clone, Debug, Default)]
pub struct Histogram {
    buckets: Vec<u64>,
    count: u64,
    sum: u64,
    min: u64,
    max: u64,
}

impl Histogram {
    /// Records a sample.
    pub fn record(&mut self, sample: u64) {
        let bucket = if sample <= 1 {
            0
        } else {
            (64 - sample.leading_zeros() - 1) as usize
        };
        if self.buckets.len() <= bucket {
            self.buckets.resize(bucket + 1, 0);
        }
        self.buckets[bucket] = self.buckets[bucket].saturating_add(1);
        if self.count == 0 || sample < self.min {
            self.min = sample;
        }
        if sample > self.max {
            self.max = sample;
        }
        self.count = self.count.saturating_add(1);
        self.sum = self.sum.saturating_add(sample);
    }

    /// Returns the total number of recorded samples.
    pub const fn count(&self) -> u64 {
        self.count
    }

    /// Returns the sum of all samples.
    pub const fn sum(&self) -> u64 {
        self.sum
    }

    /// Returns the mean of recorded samples, or 0 if no samples were recorded.
    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum as f64 / self.count as f64
        }
    }

    /// Returns the minimum recorded sample, or 0 if no samples were recorded.
    pub const fn min(&self) -> u64 {
        self.min
    }

    /// Returns the maximum recorded sample, or 0 if no samples were recorded.
    pub const fn max(&self) -> u64 {
        self.max
    }

    /// Clears the histogram for phase-based analysis.
    pub fn reset(&mut self) {
        self.buckets.clear();
        self.count = 0;
        self.sum = 0;
        self.min = 0;
        self.max = 0;
    }
}

/// A single node in the stats tree. Holds local counters, histograms, and
/// child sub-groups keyed by path segment.
#[derive(Clone, Debug, Default)]
pub struct StatGroup {
    counters: BTreeMap<&'static str, Counter>,
    histograms: BTreeMap<&'static str, Histogram>,
    children: BTreeMap<&'static str, Self>,
}

impl StatGroup {
    /// Returns a mutable reference to the counter at `path`, creating it on first access.
    pub fn counter(&mut self, path: &'static str) -> &mut Counter {
        let (head, rest) = split_path(path);
        if let Some(rest) = rest {
            self.children.entry(head).or_default().counter(rest)
        } else {
            self.counters.entry(head).or_default()
        }
    }

    /// Returns a mutable reference to the histogram at `path`, creating it on first access.
    pub fn histogram(&mut self, path: &'static str) -> &mut Histogram {
        let (head, rest) = split_path(path);
        if let Some(rest) = rest {
            self.children.entry(head).or_default().histogram(rest)
        } else {
            self.histograms.entry(head).or_default()
        }
    }

    /// Resets all counters and histograms recursively (phase boundary).
    pub fn reset(&mut self) {
        for c in self.counters.values_mut() {
            c.reset();
        }
        for h in self.histograms.values_mut() {
            h.reset();
        }
        for child in self.children.values_mut() {
            child.reset();
        }
    }

    fn dump_text(&self, prefix: &str, out: &mut dyn Write) -> io::Result<()> {
        for (name, c) in &self.counters {
            writeln!(out, "{prefix}{name} {}", c.get())?;
        }
        for (name, h) in &self.histograms {
            writeln!(
                out,
                "{prefix}{name} count={} sum={} mean={:.4} min={} max={}",
                h.count(),
                h.sum(),
                h.mean(),
                h.min(),
                h.max()
            )?;
        }
        for (name, child) in &self.children {
            let next = if prefix.is_empty() {
                format!("{name}.")
            } else {
                format!("{prefix}{name}.")
            };
            child.dump_text(&next, out)?;
        }
        Ok(())
    }
}

#[inline]
fn split_path(path: &'static str) -> (&'static str, Option<&'static str>) {
    path.find('.')
        .map_or((path, None), |idx| (&path[..idx], Some(&path[idx + 1..])))
}

/// Output format for `Stats::dump`.
#[derive(Clone, Copy, Debug)]
pub enum StatFormat {
    /// Human-readable `path.to.metric value` lines.
    Text,
}

/// Top-level statistics tree.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    /// Root of the hierarchical tree.
    pub root: StatGroup,
}

impl Stats {
    /// Creates an empty stats tree.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a mutable reference to the counter at `path`.
    ///
    /// `path` uses `.` as a separator: `"system.cpu0.cache.l1d.hits"`.
    pub fn counter(&mut self, path: &'static str) -> &mut Counter {
        self.root.counter(path)
    }

    /// Returns a mutable reference to the histogram at `path`.
    pub fn histogram(&mut self, path: &'static str) -> &mut Histogram {
        self.root.histogram(path)
    }

    /// Resets every counter and histogram in the tree.
    pub fn reset(&mut self) {
        self.root.reset();
    }

    /// Writes the stats tree to `out` in the requested format.
    ///
    /// # Errors
    ///
    /// Propagates any I/O error from the underlying writer.
    pub fn dump(&self, format: StatFormat, out: &mut dyn Write) -> io::Result<()> {
        match format {
            StatFormat::Text => self.root.dump_text("", out),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_path_resolves_and_increments() {
        let mut s = Stats::new();
        s.counter("system.cpu0.commit.insts").add(42);
        s.counter("system.cpu0.commit.insts").inc();
        assert_eq!(s.counter("system.cpu0.commit.insts").get(), 43);
    }

    #[test]
    fn histogram_records_samples() {
        let mut s = Stats::new();
        for v in [1u64, 2, 4, 8, 16, 16, 16] {
            s.histogram("system.memctrl.latency").record(v);
        }
        let h = s.histogram("system.memctrl.latency");
        assert_eq!(h.count(), 7);
        assert_eq!(h.min(), 1);
        assert_eq!(h.max(), 16);
    }

    #[test]
    fn reset_clears_everything() {
        let mut s = Stats::new();
        s.counter("a.b").add(10);
        s.histogram("a.b.h").record(5);
        s.reset();
        assert_eq!(s.counter("a.b").get(), 0);
        assert_eq!(s.histogram("a.b.h").count(), 0);
    }

    #[test]
    fn dump_text_emits_paths() {
        let mut s = Stats::new();
        s.counter("root.left").add(1);
        s.counter("root.right").add(2);
        let mut buf = Vec::new();
        s.dump(StatFormat::Text, &mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("root.left 1"));
        assert!(out.contains("root.right 2"));
    }
}
