#![no_std]
//! Platform-independent simulation contract.
//!
//! State, environment and input are explicit values. This crate owns no clock,
//! I/O or random generator. Dynamics are introduced after BPG-001 is merged.

/// Number of fixed physics ticks per simulated second, independent of rendering.
///
/// ```
/// assert_eq!(birdman_game_core::PHYSICS_HZ, 100);
/// ```
pub const PHYSICS_HZ: u32 = 100;
