// Audit M-A5 (2026-08-11): the `OutputFormatter` trait was removed — it had
// zero implementors; the production formatter is `ArtisanFormatter` /
// `MutableArtisanFormatter` (output/artisan.rs), which pre-dates the trait.
#[derive(Debug)]
pub enum OutputError {
    Serialization,
    SerialComm,
    InvalidData,
    Scheduler,
}
