use embassy_time::{Duration, Instant};
use libreroaster::config::constants::SSR_CYCLE_GUARD_MS;
use libreroaster::control::SsrCycleGuard;

fn instant_from_millis(ms: u64) -> Instant {
    Instant::from_micros(ms * 1_000)
}

#[test]
fn guard_allows_cycle_after_guard_duration() {
    let mut guard = SsrCycleGuard::new();
    let start = instant_from_millis(0);
    guard.mark_cycle(start);

    let guard_ms = SSR_CYCLE_GUARD_MS as u64;
    let allowed = instant_from_millis(guard_ms + 1);

    assert_eq!(guard.next_cycle_allowed(allowed), Ok(allowed));
}

#[test]
fn guard_rejects_commands_while_busy() {
    let mut guard = SsrCycleGuard::new();
    let start = instant_from_millis(0);
    guard.mark_cycle(start);

    let before_guard = instant_from_millis(50);
    let busy_until = guard.next_cycle_allowed(before_guard).err().unwrap();

    let expected = start + Duration::from_millis(SSR_CYCLE_GUARD_MS as u64);
    assert_eq!(busy_until, expected);
}

#[test]
fn busy_until_tracks_mark_cycle() {
    let mut guard = SsrCycleGuard::new();
    let now = instant_from_millis(250);
    guard.mark_cycle(now);

    let expected = now + Duration::from_millis(SSR_CYCLE_GUARD_MS as u64);
    assert_eq!(guard.busy_until(), expected);
}
