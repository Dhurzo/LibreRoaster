//! Regression tests for the 2026-10-16 audit fix round.
//!
//! BUG-2c-2: profile payloads are staged in three global FIFOs at parse
//! time. A dropped or refused staged-payload command must drain exactly its
//! OWN FIFO entry. The old `clear_staged_profiles()` wiped all three FIFOs,
//! silently deleting an earlier accepted command's payload that the control
//! loop had not consumed yet (one-generation profile echo).
//!
//! Kept as a single `#[test]` on purpose: the parser statics are global, so
//! spreading these assertions across parallel tests would let one test
//! drain another test's staged payloads.

#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use libreroaster::config::ArtisanCommand;
use libreroaster::input::parser::{
    discard_newest_staged_profile, discard_oldest_staged_profile, fan_profile_take,
    parse_artisan_command, ror_profile_take, take_profile,
};

/// Drain all three staging FIFOs so the test starts from a known-empty state.
fn drain_all() {
    while take_profile().is_some() {}
    while fan_profile_take().is_some() {}
    while ror_profile_take().is_some() {}
}

#[test]
fn staged_profile_drains_are_per_fifo_and_exact() {
    drain_all();

    // ── discard OLDEST removes exactly the front entry ──
    for temp in [50.0_f32, 60.0, 70.0] {
        assert_eq!(
            parse_artisan_command(&format!("PROFILE;0,{temp}")),
            Ok(ArtisanCommand::SetProfile)
        );
    }
    discard_oldest_staged_profile(&ArtisanCommand::SetProfile);
    let front = take_profile().expect("two entries must remain");
    assert_eq!(front.setpoints[0].temperature, 60.0);
    let second = take_profile().expect("one entry must remain");
    assert_eq!(second.setpoints[0].temperature, 70.0);
    assert!(take_profile().is_none());

    // ── discard NEWEST removes exactly the back entry ──
    for temp in [80.0_f32, 90.0] {
        assert_eq!(
            parse_artisan_command(&format!("PROFILE;0,{temp}")),
            Ok(ArtisanCommand::SetProfile)
        );
    }
    discard_newest_staged_profile(&ArtisanCommand::SetProfile);
    let remaining = take_profile().expect("the earlier accepted profile must survive");
    assert_eq!(remaining.setpoints[0].temperature, 80.0);
    assert!(take_profile().is_none());

    // ── cross-FIFO isolation: discarding a FANPROFILE never touches an
    //    accepted PROFILE payload (the one-generation echo) ──
    assert_eq!(
        parse_artisan_command("PROFILE;0,111"),
        Ok(ArtisanCommand::SetProfile)
    );
    assert_eq!(
        parse_artisan_command("FANPROFILE;0,42"),
        Ok(ArtisanCommand::SetFanProfile)
    );
    discard_newest_staged_profile(&ArtisanCommand::SetFanProfile);
    assert!(
        fan_profile_take().is_none(),
        "the dropped FANPROFILE took its own payload"
    );
    let survived = take_profile().expect("the accepted PROFILE must NOT be wiped");
    assert_eq!(survived.setpoints[0].temperature, 111.0);

    // ── RORPROFILE drains its own FIFO only ──
    assert_eq!(
        parse_artisan_command("RORPROFILE;0,5;60,3"),
        Ok(ArtisanCommand::SetRorProfile)
    );
    assert_eq!(
        parse_artisan_command("PROFILE;0,222"),
        Ok(ArtisanCommand::SetProfile)
    );
    discard_oldest_staged_profile(&ArtisanCommand::SetRorProfile);
    assert!(
        ror_profile_take().is_none(),
        "the refused RORPROFILE took its own payload"
    );
    let profile_kept = take_profile().expect("unrelated PROFILE must survive a RORPROFILE drain");
    assert_eq!(profile_kept.setpoints[0].temperature, 222.0);

    // ── non-staged commands are a no-op ──
    assert_eq!(
        parse_artisan_command("PROFILE;0,333"),
        Ok(ArtisanCommand::SetProfile)
    );
    discard_newest_staged_profile(&ArtisanCommand::SetHeater(50));
    discard_oldest_staged_profile(&ArtisanCommand::SetHeater(50));
    assert!(
        take_profile().is_some(),
        "a non-staged command must not drain anything"
    );
    drain_all();

    // ── drains on empty FIFOs are a no-op (no panic) ──
    discard_newest_staged_profile(&ArtisanCommand::SetProfile);
    discard_oldest_staged_profile(&ArtisanCommand::SetFanProfile);
    discard_oldest_staged_profile(&ArtisanCommand::SetRorProfile);
    discard_newest_staged_profile(&ArtisanCommand::SetRorProfile);
    assert!(take_profile().is_none());
    assert!(fan_profile_take().is_none());
    assert!(ror_profile_take().is_none());
}
