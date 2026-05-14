# LibreRoaster — Project Philosophy

---

## The Core Premise

**Coffee roasting should be accessible to everyone.**

The knowledge to roast well is freely available — what's missing is accessible, reliable, open hardware that puts precise control in the hands of hobbyists, small cafés, and coffee enthusiasts everywhere.

LibreRoaster exists to close that gap.

## Democratize the Roast

The project's goal is straightforward: **make it possible to build a competent coffee roaster using accessible, affordable components**, without sacrificing the control surface that makes modern roasting precise and repeatable.

This means:

- **Open firmware** — inspectable, auditable, modifiable. No black boxes.
- **Open toolchain** — the entire development stack is open source: Rust compiler (`rustc`), `esp-hal` and `embassy` crates, `probe-rs` / `espflash` for flashing, and standard build tooling. No proprietary IDEs, no vendor-locked SDKs, no license gates. The ESP32-C3 was chosen not only for cost and capability, but because Espressif's RISC-V chips have first-class open-source HAL support — the full register-level access, peripheral drivers, and async runtime are community-maintained and freely available.
- **Open protocols** — full compatibility with [Artisan](https://artisan-scope.org/), the de-facto standard roasting software, so operators get a professional-grade UI and logging layer for free.
- **Cheap hardware** — every component decision is filtered through "what's the least we can spend while remaining technically sound?" The ESP32-C3 was chosen because it is inexpensive, widely available, and has more than enough compute for this task. The MAX31856 was chosen because it is the industry-standard thermocouple interface with proven accuracy.

## Hardware Vision: A DIY Drum Roaster

LibreRoaster's firmware is designed around a **drum roaster topology** — two thermocouples (ET, BT), one SSR-controlled heater, one PWM fan. This maps directly to a small-batch drum roaster that can be built from off-the-shelf parts and basic metalwork.

A companion **DIY build guide** is in progress, covering:

- Complete bill of materials with cost-optimized sourcing
- Step-by-step mechanical and electrical assembly
- Plug-and-play compatibility with the LibreRoaster firmware

The intent is that someone with basic maker tools and moderate soldering ability can build a functional roaster using affordable, readily available components.

## Beyond Drum: Fluid Bed and General-Purpose Use

The firmware is tuned for drum roasting, but the architecture is intentionally general:

- **Fluid bed roasters** — hot-air popper conversions, fluid-bed builds — can be driven with minimal or no firmware changes. The control surface (heater duty cycle + fan speed + two temperature channels) maps naturally to both topologies.
- **Generic Artisan temperature logger** — even without connecting any actuators, the two MAX31856 thermocouple channels can be used as a standalone temperature acquisition device. Plug the ESP32-C3 into Artisan, read ET and BT, and you have a fully functional roast logging and curve-building setup. No firmware modification needed.

The design philosophy is: **build for drum, but don't hardcode against it.** The command set, control loop, and telemetry are all topology-agnostic at the protocol level.

## Design Principles

| Principle | What it means in practice |
|-----------|---------------------------|
| **Value-focused** | Every hardware choice is carefully balanced for performance and accessibility. Prioritizing practical, proven components over unnecessary complexity. |
| **Community-driven** | The firmware serves as a device-side controller, complementing established tools like Artisan that provide the operator interface. Building on existing standards rather than creating proprietary alternatives. |
| **Safety-aware design** | Thermal protections and monitoring are integrated into the firmware, but always with the understanding that safe hardware implementation is ultimately the builder's responsibility. |
| **Transparency first** | The codebase prioritizes clarity and maintainability. A roaster controller should be understandable and modifiable by anyone who reads the source. |
| **Open standards** | Using widely-adopted protocols (TC4 serial), common components (MAX31856, ESP32-C3, generic SSRs), and standard development tools (cargo, espflash) to ensure broad compatibility. |

## What LibreRoaster Is Not

- **Not a standalone roasting appliance.** It needs Artisan (or a compatible serial client) to drive a session.
- **Not a hardware product.** There is no kit, no PCB service, no commercial offering. This is a design and firmware that you build yourself.
- **Not certified for food safety or electrical compliance.** You are responsible for your own build's safety.

---

*This document captures the project's intent. Technical decisions that align with these principles should be preserved. Decisions that conflict with them should be questioned.*
