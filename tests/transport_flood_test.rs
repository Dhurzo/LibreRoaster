#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use libreroaster::config::ArtisanCommand;
use libreroaster::input::{CommandQueue, QueueError, COMMAND_QUEUE_SIZE};

/// Test that flooding commands within queue capacity works correctly
/// This verifies: "No dropped commands under burst load"
#[test]
fn test_flood_commands_no_drop() {
    // Create command queue with known capacity
    let mut queue: CommandQueue<ArtisanCommand, 32> = CommandQueue::new();

    // Verify initial state
    assert!(queue.is_empty());
    assert!(!queue.is_full());
    assert_eq!(queue.capacity(), 32);

    // Simulate burst of commands (within capacity)
    // Using SetHeater as a simple command variant
    for i in 0..30 {
        let cmd = ArtisanCommand::SetHeater(i as u8);
        let result = queue.try_push(cmd);
        assert!(
            result.is_ok(),
            "Command {} should fit within capacity of 32",
            i
        );
    }

    // Verify queue is full (30 items, capacity 32)
    assert!(!queue.is_empty());
    assert!(!queue.is_full()); // Not quite full, still has room
    assert_eq!(queue.len(), 30);

    // Verify FIFO order preserved
    for i in 0..30 {
        let cmd = queue.pop();
        assert!(cmd.is_some(), "Should be able to pop command {}", i);
        // Verify it's the expected command (FIFO order)
        let expected = ArtisanCommand::SetHeater(i as u8);
        assert_eq!(cmd.unwrap(), expected);
    }

    // Verify queue is empty again
    assert!(queue.is_empty());
}

/// Test that queue correctly rejects commands when full
/// This verifies: "Command multiplexer stays responsive during flood"
/// When queue is full, new commands are rejected (not queued)
#[test]
fn test_queue_reject_on_full() {
    let mut queue: CommandQueue<ArtisanCommand, 32> = CommandQueue::new();

    // Fill queue to capacity (32 items)
    for i in 0..32 {
        let cmd = ArtisanCommand::SetFan(i as u8);
        let result = queue.try_push(cmd);
        assert!(
            result.is_ok(),
            "Command {} should be accepted (filling queue)",
            i
        );
    }

    // Verify queue is now full
    assert!(queue.is_full());
    assert_eq!(queue.len(), 32);

    // Try to add one more - should reject
    let overflow_cmd = ArtisanCommand::SetHeater(100);
    let result = queue.try_push(overflow_cmd);
    assert!(result.is_err(), "Queue should reject when full");
    assert_eq!(
        result.unwrap_err(),
        QueueError::Full,
        "Error should be QueueError::Full"
    );

    // Verify existing commands are still accessible
    assert_eq!(queue.len(), 32);

    // Pop one and verify we can add again
    let popped = queue.pop();
    assert!(popped.is_some());

    // Now should be able to add
    let cmd = ArtisanCommand::SetHeater(50);
    let result = queue.try_push(cmd);
    assert!(result.is_ok(), "Should be able to add after popping");
}

/// Test that commands are processed in FIFO order under flood conditions
/// This verifies: FIFO semantics are preserved
#[test]
fn test_fifo_order_preserved() {
    let mut queue: CommandQueue<ArtisanCommand, 32> = CommandQueue::new();

    // Push different command types
    queue.try_push(ArtisanCommand::SetHeater(10)).unwrap();
    queue.try_push(ArtisanCommand::SetFan(20)).unwrap();
    queue.try_push(ArtisanCommand::IncreaseHeater).unwrap();
    queue.try_push(ArtisanCommand::DecreaseHeater).unwrap();
    queue.try_push(ArtisanCommand::StartRoast).unwrap();

    // Pop and verify order
    assert_eq!(queue.pop(), Some(ArtisanCommand::SetHeater(10)));
    assert_eq!(queue.pop(), Some(ArtisanCommand::SetFan(20)));
    assert_eq!(queue.pop(), Some(ArtisanCommand::IncreaseHeater));
    assert_eq!(queue.pop(), Some(ArtisanCommand::DecreaseHeater));
    assert_eq!(queue.pop(), Some(ArtisanCommand::StartRoast));

    // Queue should be empty
    assert!(queue.is_empty());
}

/// Test queue behavior at boundary conditions
#[test]
fn test_queue_boundary_conditions() {
    let mut queue: CommandQueue<ArtisanCommand, 4> = CommandQueue::new();

    // Test with small capacity of 4
    assert_eq!(queue.capacity(), 4);

    // Fill to capacity
    for i in 0..4 {
        assert!(queue.try_push(ArtisanCommand::SetHeater(i as u8)).is_ok());
    }

    // Should be full
    assert!(queue.is_full());
    assert!(!queue.is_empty());
    assert_eq!(queue.len(), 4);

    // Reject overflow
    assert!(queue.try_push(ArtisanCommand::SetHeater(100)).is_err());

    // Pop all
    for _ in 0..4 {
        assert!(queue.pop().is_some());
    }

    // Should be empty
    assert!(queue.is_empty());
    assert!(!queue.is_full());
}

/// Test mixed command types under flood
#[test]
fn test_mixed_command_types_flood() {
    let mut queue: CommandQueue<ArtisanCommand, 32> = CommandQueue::new();

    // Mix of different command types
    let commands = vec![
        ArtisanCommand::ReadStatus,
        ArtisanCommand::SetHeater(25),
        ArtisanCommand::SetFan(50),
        ArtisanCommand::StartRoast,
        ArtisanCommand::IncreaseHeater,
        ArtisanCommand::DecreaseHeater,
        ArtisanCommand::EmergencyStop,
        ArtisanCommand::SetHeater(75),
    ];

    // Push all commands
    for cmd in &commands {
        assert!(queue.try_push(*cmd).is_ok());
    }

    // Verify all were queued
    assert_eq!(queue.len(), commands.len());

    // Pop and verify order matches input
    for expected in &commands {
        let actual = queue.pop();
        assert_eq!(actual, Some(*expected));
    }
}

/// Test that default queue size is 32 as specified
#[test]
fn test_default_queue_size() {
    // The COMMAND_QUEUE_SIZE constant should be 32
    assert_eq!(COMMAND_QUEUE_SIZE, 32);

    // Create queue using the default size
    let queue: CommandQueue<ArtisanCommand, { COMMAND_QUEUE_SIZE }> = CommandQueue::new();
    assert_eq!(queue.capacity(), 32);
}

/// Stress test: Fill and drain queue repeatedly
/// This simulates rapid command flood and processing
#[test]
fn test_rapid_flood_drain() {
    let mut queue: CommandQueue<ArtisanCommand, 32> = CommandQueue::new();

    // Multiple rounds of flood and drain
    for round in 0..10 {
        // Flood
        for i in 0..30 {
            let cmd = ArtisanCommand::SetHeater((round * 10 + i) as u8);
            assert!(
                queue.try_push(cmd).is_ok(),
                "Round {}: Should accept command {}",
                round,
                i
            );
        }

        // Drain
        for _ in 0..30 {
            assert!(
                queue.pop().is_some(),
                "Round {}: Should have commands to pop",
                round
            );
        }

        // Verify empty
        assert!(
            queue.is_empty(),
            "Round {}: Should be empty after drain",
            round
        );
    }
}

/// Test that queue handles all ArtisanCommand variants
#[test]
fn test_all_command_variants() {
    let mut queue: CommandQueue<ArtisanCommand, 16> = CommandQueue::new();

    // All variants of ArtisanCommand
    let commands = [
        ArtisanCommand::ReadStatus,
        ArtisanCommand::StartRoast,
        ArtisanCommand::SetHeater(50),
        ArtisanCommand::SetFan(50),
        ArtisanCommand::SetFanSpeed(50, false),
        ArtisanCommand::EmergencyStop,
        ArtisanCommand::IncreaseHeater,
        ArtisanCommand::DecreaseHeater,
        ArtisanCommand::Chan(100),
        ArtisanCommand::Units(true),
        ArtisanCommand::Filt(1),
    ];

    // Push all variants
    for cmd in &commands {
        let result = queue.try_push(*cmd);
        assert!(result.is_ok(), "Should accept command variant: {:?}", cmd);
    }

    // Verify all were queued
    assert_eq!(queue.len(), commands.len());

    // Pop all
    for expected in &commands {
        assert_eq!(queue.pop(), Some(*expected));
    }
}
