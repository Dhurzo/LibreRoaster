# Architecture Research: ARTISAN+ Integration

## System Architecture

### Component Diagram

```
Artisan (PC)
    |
    | UART (115200, 8N1)
    |
ESP32-C3
    |
    +--- uart/ (hardware/driver)
    |       |
    |       +--- tasks.rs (reader/writer tasks)
    |       |
    |       +--- driver.rs (UART driver wrapper)
    |
    +--- input/
    |       |
    |       +--- parser.rs (command parsing)
    |       |
    |       +--- mod.rs (ArtisanInput interface)
    |
    +--- control/
    |       |
    |       +--- handlers.rs (ArtisanCommandHandler)
    |       |
    |       +--- roaster_refactored.rs (command processing)
    |
    +--- output/
            |
            +--- artisan.rs (response formatting)
            |
            +--- mod.rs (OutputFormatter trait)
```

### Data Flow

**Incoming Commands (Artisan → ESP32)**
```
UART RX → uart_reader_task → COMMAND_PIPE → artisan_uart_handler_task
        → parse_artisan_command() → ArtisanCommand
        → roaster.process_artisan_command() → state change
```

**Outgoing Responses (ESP32 → Artisan)**
```
roaster status → ArtisanFormatter.format() → CSV string
        → uart_writer_task → UART TX
```

### Key Components

1. **uart_reader_task**: Reads bytes from UART, parses commands
2. **uart_writer_task**: Writes responses to UART
3. **artisan_uart_handler_task**: Processes commands, updates roaster state
4. **ArtisanCommandHandler**: Manages artisan control mode, heater/fan setpoints

## Test Architecture Needs

### Unit Tests
- `parser.rs` tests: ✓ existing, comprehensive
- `artisan.rs` formatter tests: ✗ missing

### Integration Tests
- Command → Response flow: ✗ missing
- UART mock: ✗ missing
- State machine transitions: ✗ missing

---

*Research date: 2026-02-04*
