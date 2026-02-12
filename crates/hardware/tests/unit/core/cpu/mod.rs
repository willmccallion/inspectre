//! # CPU Unit Tests
//!
//! This module contains unit tests for CPU operations including trap handling,
//! CSR operations, and memory access.

/// Tests for trap and exception handling.
pub mod trap_handling;

/// Tests for CPU execution and pipeline coordination.
pub mod execution;

/// Tests for memory access and cache simulation.
pub mod memory;
