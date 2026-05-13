# LibreRoaster — Project Philosophy

---

## The Core Premise

**Coffee roasting should not be a luxury.**

Commercial roasters cost thousands. Even entry-level machines with basic PID control carry a premium that puts consistent, repeatable roasting out of reach for most hobbyists, small cafés, and developing-region producers. Meanwhile, the knowledge to roast well is freely available — what's missing is accessible, reliable, open hardware.

LibreRoaster exists to close that gap.

## Democratize the Roast

The project's goal is straightforward: **make it possible to build a competent coffee roaster for a fraction of the cost of commercial options**, without sacrificing the control surface that makes modern roasting precise and repeatable.

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

The intent is that someone with basic maker tools and moderate soldering ability can build a functional roaster for a fraction of commercial pricing.

## Beyond Drum: Fluid Bed and General-Purpose Use

The firmware is tuned for drum roasting, but the architecture is intentionally general:

- **Fluid bed roasters** — hot-air popper conversions, fluid-bed builds — can be driven with minimal or no firmware changes. The control surface (heater duty cycle + fan speed + two temperature channels) maps naturally to both topologies.
- **Generic Artisan temperature logger** — even without connecting any actuators, the two MAX31856 thermocouple channels can be used as a standalone temperature acquisition device. Plug the ESP32-C3 into Artisan, read ET and BT, and you have a fully functional roast logging and curve-building setup. No firmware modification needed.

The design philosophy is: **build for drum, but don't hardcode against it.** The command set, control loop, and telemetry are all topology-agnostic at the protocol level.

## Design Principles

| Principle | What it means in practice |
|-----------|---------------------------|
| **Cost-conscious** | Every hardware choice justified by cost-to-performance ratio. No prestige components. |
| **Artisan-first** | The firmware is a device-side controller. Artisan is the operator's interface. Don't reinvent the UI. |
| **Safety-critical realism** | Thermal cutoffs, watchdogs, and guards exist in firmware, but are treated as *last resort*. Safe hardware design is the operator's responsibility. |
| **Inspectability over abstraction** | The codebase favors clarity over cleverness. A roaster controller should be understandable by anyone reading the source. |
| **No vendor lock-in** | Standard protocols (TC4 serial), standard components (MAX31856, ESP32-C3, generic SSRs), standard tools (Artisan, cargo, espflash). |

## What LibreRoaster Is Not

- **Not a standalone roasting appliance.** It needs Artisan (or a compatible serial client) to drive a session.
- **Not a hardware product.** There is no kit, no PCB service, no commercial offering. This is a design and firmware that you build yourself.
- **Not certified for food safety or electrical compliance.** You are responsible for your own build's safety.

---

*This document captures the project's intent. Technical decisions that align with these principles should be preserved. Decisions that conflict with them should be questioned.*
