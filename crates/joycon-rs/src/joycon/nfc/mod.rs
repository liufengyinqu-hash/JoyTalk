//! NFC tag detection for Joy-Con (R) — protocol from CTCaer / bettse RE notes.

mod session;

pub use session::{
    coalesce_nfc_samples, nfc_state_error, nfc_state_ready, nfc_state_scanning, parse_nfc_state,
    peek_nfc_report, should_restart_after_tag_removal, EnableProgress, NfcSample, NfcSession,
};
