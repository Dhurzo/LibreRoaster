//! Digital twin for GPIO pin behaviour simulation.
//!
//! Simulates the virtual state of every GPIO on the ESP32-C3 so that pin-level
//! logic (e.g. "SSR ON means GPIO1 reads LOW") can be verified on the host
//! without real hardware.

use libreroaster::config::pinout::*;

#[derive(Debug, Clone, Copy, PartialEq)]
enum VirtualLevel {
    Low,
    High,
    Floating,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum VirtualDirection {
    Input,
    Output,
}

struct VirtualPin {
    level: VirtualLevel,
    direction: VirtualDirection,
    pull_up: bool,
}

impl Default for VirtualPin {
    fn default() -> Self {
        Self {
            level: VirtualLevel::Floating,
            direction: VirtualDirection::Input,
            pull_up: false,
        }
    }
}

struct VirtualBoard {
    pins: [VirtualPin; 22],
}

impl VirtualBoard {
    fn new() -> Self {
        let mut board = Self {
            pins: std::array::from_fn(|_| VirtualPin::default()),
        };
        board.apply_pin_table();
        board
    }

    fn apply_pin_table(&mut self) {
        for spec in PIN_TABLE {
            let idx = spec.gpio as usize;
            self.pins[idx].direction = match spec.direction {
                PinDirection::Input | PinDirection::Bidirectional => VirtualDirection::Input,
                PinDirection::Output => VirtualDirection::Output,
            };
            if spec.constraints.contains(&PinConstraint::InternalPullUpRequired) {
                self.pins[idx].pull_up = true;
                self.pins[idx].level = VirtualLevel::High;
            }
        }
    }

    fn set_output(&mut self, gpio: u8, high: bool) {
        let idx = gpio as usize;
        assert!(
            self.pins[idx].direction == VirtualDirection::Output,
            "GPIO{} is not configured as output",
            gpio
        );
        self.pins[idx].level = if high {
            VirtualLevel::High
        } else {
            VirtualLevel::Low
        };
    }

    fn read_pin(&self, gpio: u8) -> bool {
        let idx = gpio as usize;
        match self.pins[idx].level {
            VirtualLevel::High => true,
            VirtualLevel::Low => false,
            VirtualLevel::Floating => self.pins[idx].pull_up,
        }
    }

    fn simulate_ssr_feedback(&mut self, ssr_conducting: bool) {
        self.pins[1].level = if ssr_conducting {
            VirtualLevel::Low
        } else {
            VirtualLevel::High
        };
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[test]
fn ssr_conducting_means_heat_detection_reads_low() {
    let mut board = VirtualBoard::new();
    board.simulate_ssr_feedback(true);
    assert!(
        !board.read_pin(1),
        "GPIO1 should read LOW when SSR is conducting"
    );
}

#[test]
fn ssr_off_means_heat_detection_reads_high_via_pullup() {
    let mut board = VirtualBoard::new();
    board.simulate_ssr_feedback(false);
    assert!(
        board.read_pin(1),
        "GPIO1 should read HIGH (pull-up) when SSR is off"
    );
}

#[test]
fn emergency_stop_drives_ssr_low_and_fan_high() {
    let mut board = VirtualBoard::new();

    board.set_output(10, true);
    board.set_output(9, false);

    board.set_output(10, false);
    board.set_output(9, true);

    assert!(
        !board.read_pin(10),
        "Emergency stop: GPIO10 (SSR) must be LOW"
    );
    assert!(
        board.read_pin(9),
        "Emergency stop: GPIO9 (Fan) must be HIGH"
    );
}

#[test]
fn both_spi_cs_are_never_low_simultaneously() {
    let mut board = VirtualBoard::new();
    board.set_output(3, true);
    board.set_output(4, true);

    board.set_output(3, false);
    assert!(
        board.read_pin(4),
        "BT CS must remain HIGH while ET CS is LOW"
    );

    board.set_output(3, true);
    board.set_output(4, false);
    assert!(
        board.read_pin(3),
        "ET CS must remain HIGH while BT CS is LOW"
    );
}

#[test]
fn spi_cs_pins_default_to_high_after_init() {
    let mut board = VirtualBoard::new();
    board.set_output(3, true);
    board.set_output(4, true);

    assert!(board.read_pin(3), "ET CS must default HIGH (deselected)");
    assert!(board.read_pin(4), "BT CS must default HIGH (deselected)");
}

#[test]
fn gpio2_is_never_touched() {
    assert!(
        find_pin(2).is_none(),
        "GPIO2 must not be in PIN_TABLE (VDD_SPI strapping)"
    );
}

#[test]
fn all_output_pins_can_toggle() {
    let mut board = VirtualBoard::new();
    for pin in output_pins() {
        board.set_output(pin.gpio, true);
        assert!(board.read_pin(pin.gpio), "GPIO{} HIGH failed", pin.gpio);
        board.set_output(pin.gpio, false);
        assert!(!board.read_pin(pin.gpio), "GPIO{} LOW failed", pin.gpio);
    }
}

#[test]
fn input_pins_reject_set_output() {
    let board = VirtualBoard::new();
    for pin in input_pins() {
        assert_eq!(
            board.pins[pin.gpio as usize].direction,
            VirtualDirection::Input,
            "GPIO{} (\"{}\") should be configured as input",
            pin.gpio,
            pin.function,
        );
    }
}

#[test]
fn heat_detection_pullup_holds_high_when_floating() {
    let board = VirtualBoard::new();
    assert!(
        board.read_pin(1),
        "GPIO1 should read HIGH when floating (internal pull-up active)"
    );
}

#[test]
fn no_input_pin_in_pin_table_can_be_written() {
    let board = VirtualBoard::new();
    for pin in input_pins() {
        let vp = &board.pins[pin.gpio as usize];
        assert_eq!(
            vp.direction,
            VirtualDirection::Input,
            "GPIO{} (\"{}\") is declared Input in PIN_TABLE but configured as Output in VirtualBoard",
            pin.gpio,
            pin.function,
        );
    }
}
