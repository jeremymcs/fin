// Fin — I/O Adapters (Transport Layer)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

pub mod agent_io;
pub mod channel_io;
pub mod headless;
pub mod mcp;
pub mod print;
pub mod print_io;
pub mod rpc;

#[cfg(feature = "http")]
pub mod http;
