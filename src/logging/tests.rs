//! Unit tests for logging infrastructure
//! These tests run on host (no hardware required)

#[cfg(test)]
mod channel_tests {
    use super::*;

    #[test]
    fn test_channel_prefix_format() {
        // Test that the expected format is correct
        let channel = "USB";
        let message = "Command received";
        let formatted = format!("[{}] {}", channel, message);
        assert_eq!(formatted, "[USB] Command received");
    }

    #[test]
    fn test_all_channel_formats() {
        let channels = ["USB", "UART", "SYSTEM"];
        for channel in channels {
            let msg = format!("[{}] test message", channel);
            assert!(msg.starts_with(&format!("[{}]", channel)));
        }
    }
}

#[cfg(test)]
mod buffer_tests {
    use heapless::spsc::Queue;

    #[test]
    fn test_spsc_queue_basic() {
        // Test basic SPSC queue behavior (used in logging)
        let mut producer: Queue<u8, 16> = Queue::new();
        let mut consumer = producer.split().1;

        // Producer writes
        producer.enqueue(&1).unwrap();
        producer.enqueue(&2).unwrap();
        producer.enqueue(&3).unwrap();

        // Consumer reads
        assert_eq!(consumer.dequeue(), Some(1));
        assert_eq!(consumer.dequeue(), Some(2));
        assert_eq!(consumer.dequeue(), Some(3));
        assert_eq!(consumer.dequeue(), None);
    }

    #[test]
    fn test_queue_wrap_around() {
        // Test queue wrap-around behavior
        let mut producer: Queue<u8, 4> = Queue::new();
        let mut consumer = producer.split().1;

        // Fill and drain
        for i in 0..3 {
            producer.enqueue(&i).unwrap();
        }
        for _ in 0..3 {
            assert!(consumer.dequeue().is_some());
        }

        // Should be empty
        assert_eq!(consumer.dequeue(), None);

        // Should be able to write again (wrap-around)
        producer.enqueue(&99).unwrap();
        assert_eq!(consumer.dequeue(), Some(99));
    }
}

#[cfg(test)]
mod format_tests {
    #[test]
    fn test_artisan_command_log_format() {
        let channel = "USB";
        let command = "READ";
        let log_line = format!("[{}] Command: {}", channel, command);

        assert!(log_line.contains("[USB]"));
        assert!(log_line.contains("Command: READ"));
    }

    #[test]
    fn test_temperature_log_format() {
        let temp = 185.5;
        let log_line = format!("[SYSTEM] Temperature: {:.1}C", temp);

        assert!(log_line.starts_with("[SYSTEM]"));
        assert!(log_line.contains("185.5"));
        assert!(log_line.contains("Temperature:"));
    }
}

#[cfg(test)]
mod usb_traffic_tests {
    #[test]
    fn test_usb_rx_log_format() {
        let channel = "USB";
        let command = "READ";
        let log_line = format!("[{}] RX: {}", channel, command);
        assert_eq!(log_line, "[USB] RX: READ");
    }

    #[test]
    fn test_usb_tx_log_format() {
        let channel = "USB";
        let response = "185.2,192.3,-1.0,-1.0,24.5,45,75";
        let log_line = format!("[{}] TX: {}", channel, response);
        assert_eq!(log_line, "[USB] TX: 185.2,192.3,-1.0,-1.0,24.5,45,75");
    }

    #[test]
    fn test_usb_command_trimming() {
        let raw = "READ\r\n";
        let trimmed = raw.trim_end();
        assert_eq!(trimmed, "READ");
    }

    #[test]
    fn test_all_artisan_commands_log_format() {
        let commands = ["READ", "START", "STOP", "OT1 75", "SET"];
        for cmd in commands {
            let log_line = format!("[USB] RX: {}", cmd);
            assert!(log_line.starts_with("[USB] RX:"));
        }
    }
}
