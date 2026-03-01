# Phase 77: Memory Optimization - Research

**Phase:** 77
**Requirement:** PERF-03
**Gathered:** 2026-02-28

## Task Summary

Replace `Vec<f32>` with `heapless::Deque<f32, 5>` in ArtisanFormatter and MutableArtisanFormatter. Replace `alloc::format!` with `core::write!` using `heapless::String`.

## Codebase Analysis

### Current Implementation
- `ArtisanFormatter.bt_history`: `Vec<f32>` (line 25 in src/output/artisan.rs)
- `MutableArtisanFormatter.bt_history`: `Vec<f32>` (line 193 in src/output/artisan.rs)
- Uses `alloc::format!` macro for string formatting (line 16)
- heapless crate already in Cargo.toml: `heapless = "0.9.2"`

### Changes Required
1. Import `heapless::Deque` instead of `alloc::vec::Vec`
2. Change `bt_history: Vec<f32>` to `bt_history: Deque<f32, 5>`
3. Replace `.clear()` with manual clear or `Deque::new()`
4. Replace `history.remove(0)` with `pop_front()` (FIFO behavior)
5. Replace `alloc::format!` with `heapless::String` + `core::write!`

## heapless::Deque API

```rust
use heapless::Deque;

// Create: Deque::<f32, 5>::new()
// Push: deque.push_back(value)  // returns Result<(), ()> when full
// Pop: deque.pop_front()       // returns Option<T>
// Clear: equivalent to reinitialize
// Len: deque.len()
// Capacity: deque.capacity()
```

Note: Deque::push_back returns `Result<(), ()>` - fails when full. For fixed-size FIFO, the current behavior of removing oldest when full should be preserved.

## Key Considerations

1. **FIFO behavior**: Current code uses `history.remove(0)` when len >= 5. Deque should use `pop_front()` then `push_back()`.

2. **Capacity**: 5 elements - matches current behavior exactly.

3. **String formatting**: The `alloc::format!` usage is in hot path. Need to replace with `heapless::String` and `core::write!` macro.

## Verification

- All existing tests must pass
- No heap allocation in format() method path

---

*No external research needed - mechanical refactoring with clear patterns.*
