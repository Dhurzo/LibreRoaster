---
plan: 51-01
phase: 51-documentation
status: complete
wave: 1
commits:
  - hash: 257dc55
    message: "docs(51-01): update README protocol to 4-value format"
---

## Summary

Updated README.md Protocol section to match PROTOCOL.md's 4-value format (ET,BT,HEATER,FAN).

### Changes Made

- Updated READ Response Format section (lines 99-115) to show 4-value CSV format
- Added Type and Unit columns to the field table
- Changed example from `185.2,192.3,-1,-1,24.5,45,75` to `185.3,201.4,45,80`
- Updated command description in Supported Artisan Commands table

### Verification

- `grep -n "ET,BT" README.md` ✓
- `grep -n "185.3,201.4,45,80" README.md` ✓

### Success Criteria Met

- [x] README.md states READ returns 4 values
- [x] README.md examples show ET,BT,HEATER,FAN format  
- [x] README.md matches PROTOCOL.md exactly
