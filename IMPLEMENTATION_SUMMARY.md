# Dual-Channel Artisan Communication - Implementation Summary

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      Artisan Scope (PC)                          │
└─────────────────────────────┬───────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              │                               │
         USB CDC (/dev/ttyACM0)         UART0 (GPIO20/21)
              │                               │
              ▼                               ▼
    ┌─────────────────┐             ┌─────────────────┐
    │ usb_reader_task  │             │ uart_reader_task │
    │ usb_writer_task │             │ uart_writer_task │
    └────────┬────────┘             └────────┬────────┘
             │                               │
             └───────────────┬───────────────┘
                             │
                             ▼
              ┌─────────────────────────────┐
              │   CommandMultiplexer         │
              │   - active_channel           │
              │   - last_command_time       │
              │   - timeout: 60s            │
              └─────────────┬───────────────┘
                            │
                            ▼
              ┌─────────────────────────────┐
              │   Artisan Command Channel    │
              └─────────────┬───────────────┘
                            │
                            ▼
              ┌─────────────────────────────┐
              │   control_loop_task         │
              │   - Processes commands      │
              │   - Updates heater/fan     │
              └─────────────┬───────────────┘
                            │
                            ▼
              ┌─────────────────────────────┐
              │   Output Channel            │
              └─────────────┬───────────────┘
                            │
              ┌─────────────┴─────────────┐
              │                           │
              ▼                           ▼
    ┌─────────────────┐         ┌─────────────────┐
    │ dual_output_task │         │ dual_output_task │
    │ (writes to USB) │         │ (writes to UART)│
    └─────────────────┘         └─────────────────┘
```

## Key Files

| File | Purpose |
|------|---------|
| `src/hardware/usb_cdc/mod.rs` | USB CDC module definition |
| `src/hardware/usb_cdc/driver.rs` | USB Serial JTAG driver |
| `src/hardware/usb_cdc/tasks.rs` | USB reader/writer tasks |
| `src/input/multiplexer.rs` | Channel multiplexer logic |
| `src/application/tasks.rs` | dual_output_task |
| `src/application/service_container.rs` | Multiplexer state |
| `src/main.rs` | USB CDC initialization |

## Multiplexer State Machine

```
                    ┌─────────────────┐
                    │      None       │◄────────────────────────┐
                    └────────┬────────┘                         │
                             │                                 │
              First command  │                                 │ Timeout
                    on any  │ channel                         │ (60s)
                             ▼                                 │
                    ┌─────────────────┐                       │
           ┌────────┤    Usb/Uart     │                       │
           │        │    (active)     │                       │
           │        └────────┬────────┘                       │
           │                 │                                │
           │    Same channel │ command                        │ Other channel
           │                 │                                │ command
           │                 ▼                                │
           │        ┌─────────────────┐                      │
           │        │  Update timer   │──────────────────────┘
           │        │  Keep active   │
           │        └─────────────────┘
           │
           │    Other channel
           │    command received
           └────► Ignore command
                  Log warning
```

## Channel Priority Logic

1. **First command wins**: El primer comando recibido en cualquier canal establece ese canal como activo
2. **Same channel OK**: Comandos del canal activo se procesan normalmente
3. **Different channel ignored**: Comandos en el canal inactivo se ignoran
4. **Timeout resets**: 60 segundos sin comandos → canal activo = None

## Implementation Details

### USB Reader Task
```rust
async fn usb_reader_task() {
    loop {
        if let Some(usb) = get_usb_cdc_driver() {
            let len = usb.read_bytes(&mut rbuf).await?;
            if len > 0 {
                process_usb_command_data(&rbuf[..len]);
            }
        }
        Timer::after(Duration::from_millis(10)).await;
    }
}
```

### Multiplexer Channel Selection
```rust
fn on_command_received(&mut self, channel: CommChannel) -> bool {
    match self.active_channel {
        CommChannel::None => {
            self.active_channel = channel;
            true
        }
        current if current == channel => {
            // Same channel - update timer if timeout
            true
        }
        _ => {
            // Different channel - ignore
            false
        }
    }
}
```

### Dual Output Task
```rust
async fn dual_output_task() {
    loop {
        if let Ok(data) = output_channel.try_receive() {
            let channel = critical_section::with(|cs| {
                mux.borrow(cs).get_active_channel()
            });

            match channel {
                CommChannel::Usb => usb.write_bytes(data).await,
                CommChannel::Uart => uart.write_bytes(data).await,
                CommChannel::None => /* discard */,
            }
        }
    }
}
```

## Test Coverage

| Test Case | Description | Expected Result |
|-----------|-------------|-----------------|
| USB first | Connect USB, send READ | Response on USB |
| UART first | Connect UART, send READ | Response on UART |
| Channel switch | USB active, command on UART | UART ignored, log warning |
| Timeout | No commands for 60s | Channel resets to None |
| READ response | Request temperatures | ET,BT,Power,Fan format |
| START/STOP | Control loop | State changes correctly |
| OT1/IO3 | Manual control | Output values set |
| Error handling | Invalid commands | ERR response |

## Logs to Monitor

```
[INFO] Artisan command received on USB, switching active channel to USB
[INFO] Artisan command received on UART, switching active channel to UART
[INFO] Ignoring artisan command on UART, active channel is USB
[INFO] No artisan commands for 60s, switching active channel to None
```

## Verification Commands

```bash
# Build
cargo build --release

# Flash
espflash flash --monitor --chip esp32c3 target/riscv32imac-unknown-none-elf/release/libreroaster

# Monitor logs
espflash monitor

# Test with minicom
minicom -D /dev/ttyACM0 -b 115200

# Test with screen
screen /dev/ttyACM0 115200
```
