# Pitfalls Research: ARTISAN+ Testing

## Common Issues

### 1. UART Buffer Overflow
**Symptom**: Commands truncated or lost during high-throughput
**Prevention**: Buffer size adequate for command frequency

### 2. Parser State Machine Issues
**Symptom**: Commands not parsed correctly
**Prevention**: Comprehensive parser tests covering all command formats

### 3. Response Format Mismatch
**Symptom**: Artisan doesn't understand firmware responses
**Prevention**: Verify output format matches ARTISAN+ specification exactly

### 4. Example File API Mismatch
**Location**: `examples/artisan_test.rs`
**Issue**: Method signature `format(&status, 25.0)` doesn't match actual `format(&status)`
**Fix**: Update example or remove

### 5. Missing Error Propagation
**Symptom**: UART errors silently ignored
**Prevention**: Add error handling in UART tasks

## Test-Specific Pitfalls

### 1. No Hardware Testing
**Issue**: Unit tests can't catch hardware-specific bugs
**Mitigation**: Focus on protocol correctness; hardware test in 2 days will catch timing/electrical issues

### 2. Async Testing Complexity
**Issue**: UART tasks are async, hard to test
**Mitigation**: Test parser/formatter synchronously; mock UART for integration tests

### 3. Floating Point Formatting
**Issue**: Temperature values may have inconsistent decimal places
**Mitigation**: Test output formatting with known inputs

## Recommended Test Order

1. Fix example file API mismatch
2. Add formatter tests (ArtisanFormatter, MutableArtisanFormatter)
3. Add boundary value tests for parser
4. Add integration test for READ command → response
5. Verify with hardware in 2 days

---

*Research date: 2026-02-04*
