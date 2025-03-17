# SpacetimeDB Music Player Server

This repository contains the server-side code for the SpacetimeDB Music Player, a Rust-based music server module that interfaces with SpacetimeDB.

## Project Structure

- `/src` - Rust source code for the SpacetimeDB module
  - `/src/lib.rs` - Main module file with database schema and reducer functions
- `Cargo.toml` - Rust dependencies and project configuration

## Features

- Music track management
- User authentication
- Playlist functionality
- Integration with SpacetimeDB

## Technologies Used

- Rust programming language
- SpacetimeDB for database functionality
- WebAssembly for client-server interaction

## Getting Started

1. Install Rust and Cargo if not already installed
2. Install SpacetimeDB CLI
3. Build the project:
   ```
   cargo build
   ```
4. Publish to a local SpacetimeDB instance:
   ```
   spacetime publish
   ```

## Client Integration

This server component works with the [SpacetimeDB Music Player Client](https://github.com/dylantarre/spaceship) to provide a complete music player application.
