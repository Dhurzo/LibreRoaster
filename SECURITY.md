# Security Policy

## Supported Versions

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a Vulnerability

This project is maintained by volunteers. We take security vulnerabilities seriously.

To report a vulnerability:

1. **Open a GitHub Security Advisory** at https://github.com/Dhurzo/LibreRoaster/security/advisories/new
2. Alternatively, open a public [GitHub Issue](https://github.com/Dhurzo/LibreRoaster/issues) for non-critical issues

You should receive a response within 5 business days. If you don't, please follow up on the issue.

**Please do NOT report security vulnerabilities via public GitHub Issues for critical vulnerabilities** — use the Security Advisory feature.

## What to Include

- Description of the vulnerability
- Steps to reproduce
- Affected versions
- Potential impact
- Any suggested fix (if known)

## Scope

This security policy covers:

- The firmware binary (embedded Rust code running on ESP32-C3)
- Build and deployment tooling
- Serial protocol implementation

Out of scope:

- Physical security of the roasting hardware
- Mains electrical safety (covered by hardware design, not firmware)
- Third-party dependencies (report to their maintainers)

## Safety-Critical Components

This firmware controls high-temperature hardware. The following components have safety implications:

- Over-temperature cutoff (260°C threshold)
- Rate-of-rise protection (30°C/min max)
- Dual watchdog (software + hardware RWDT)
- Heat-source detection
- SSR output validation
- Max roast time (30 min hard limit)

Vulnerabilities in these components are treated as HIGH priority.
