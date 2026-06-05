//! NFC session for Joy-Con (R): tag presence + UID (NTAG / Mifare / Amiibo).

use crate::joycon::driver::{JoyConDriver, SubCommand, SubCommandReply};
use crate::result::{JoyConError, JoyConResult};
use joycon_sys::common::InputReportId;
use joycon_sys::mcu::{MCUCommand, MCUReport, MCUReportId, MCURequest, MCURequestEnum, MCUMode};
use joycon_sys::output::OutputReport;
use joycon_sys::InputReport;
use std::thread;
use std::time::{Duration, Instant};

const READ_TIMEOUT_MS: i32 = 20;
const MCU_WAIT_BUDGET: Duration = Duration::from_secs(6);
const ENABLE_STEP_BUDGET: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnableProgress {
    Continue,
    Done,
}
/// MCU stream mode `0x02`: include NFC/IR payload in `0x31` input reports (required).
const MCU_STREAM_NFC: u8 = 0x02;
const NFC_POLL_PAYLOAD: &[u8] = &[0x01, 0x00, 0x00, 0x08, 0x05, 0x00, 0x00, 0x00, 0x2c, 0x01];
const NFC_DISCOVERY_PAYLOAD: &[u8] = &[0x04, 0x00, 0x00, 0x08];
const NFC_CANCEL_PAYLOAD: &[u8] = &[0x00, 0x00, 0x00, 0x08];
const NFC_STOP_PAYLOAD: &[u8] = &[0x02, 0x00, 0x00, 0x08];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NfcSample {
    pub state: u8,
    pub tag_present: bool,
    /// MCU reports tag info in the field but UID read is not finalized (state != 0x09).
    pub tag_detected: bool,
    pub tag_type: u8,
    pub uid: Vec<u8>,
}

impl NfcSample {
    pub fn uid_hex(&self) -> String {
        self.uid.iter().map(|b| format!("{b:02x}")).collect()
    }
}

pub struct NfcSession {
    active: bool,
    enable_step: Option<u8>,
    last_state: u8,
    /// Previous tick had a confirmed tag (state 0x09 + UID).
    tag_present_last: bool,
    /// Consecutive idle-polling ticks after a tag was present (debounce removal).
    removal_idle_streak: u8,
    /// Consecutive NFC IC error ticks (e.g. 0x07) without a readable tag.
    error_streak: u8,
    last_error_recover: Option<Instant>,
}

const REMOVAL_IDLE_STREAK: u8 = 2;
const ERROR_RECOVERY_STREAK: u8 = 15;
const ERROR_RECOVERY_SLEEP_MS: u64 = 100;
const ERROR_RECOVERY_COOLDOWN: Duration = Duration::from_secs(2);

impl NfcSession {
    pub fn new() -> Self {
        Self {
            active: false,
            enable_step: None,
            last_state: 0,
            tag_present_last: false,
            removal_idle_streak: 0,
            error_streak: 0,
            last_error_recover: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn is_enabling(&self) -> bool {
        self.enable_step.is_some()
    }

    pub fn begin_enable(&mut self) {
        self.enable_step = Some(0);
        self.active = false;
    }

    /// Advance NFC enable by at most `budget` without blocking the caller for long.
    pub fn advance_enable<D: JoyConDriver>(
        &mut self,
        driver: &mut D,
        budget: Duration,
    ) -> JoyConResult<EnableProgress> {
        let Some(step) = self.enable_step else {
            return Ok(if self.active {
                EnableProgress::Done
            } else {
                EnableProgress::Continue
            });
        };
        let deadline = Instant::now() + budget;
        let mut step = step;
        while Instant::now() < deadline {
            match step {
                0 => {
                    set_report_mode(driver, InputReportId::StandardFullMCU)?;
                    self.enable_step = Some(1);
                    step = 1;
                }
                1 => {
                    poll_mcu_stream(driver, MCU_STREAM_NFC)?;
                    call_subcmd_bytes(
                        driver,
                        SubCommand::Set_NFC_IR_MCUState as u8,
                        &mcu_mode_bytes(MCUMode::Standby),
                    )?;
                    self.enable_step = Some(2);
                    step = 2;
                }
                2 => {
                    if wait_mcu_status_until(driver, MCUMode::Standby, deadline)? {
                        self.enable_step = Some(3);
                        step = 3;
                    } else {
                        return Ok(EnableProgress::Continue);
                    }
                }
                3 => {
                    send_mcu_cmd(driver, MCUCommand::set_mcu_mode(MCUMode::NFC))?;
                    self.enable_step = Some(4);
                    step = 4;
                }
                4 => {
                    if wait_mcu_status_until(driver, MCUMode::NFC, deadline)? {
                        self.enable_step = Some(5);
                        step = 5;
                    } else {
                        return Ok(EnableProgress::Continue);
                    }
                }
                5 => {
                    send_nfc_request(driver, NFC_DISCOVERY_PAYLOAD)?;
                    send_nfc_request(driver, NFC_POLL_PAYLOAD)?;
                    self.enable_step = Some(6);
                    step = 6;
                }
                6 => {
                    if wait_nfc_ready_until(driver, deadline)? {
                        self.last_state = 0x01;
                        self.tag_present_last = false;
                        self.removal_idle_streak = 0;
                        self.error_streak = 0;
                        self.active = true;
                        self.enable_step = None;
                        log::info!("[joycon-rs] NFC session enabled");
                        return Ok(EnableProgress::Done);
                    }
                    return Ok(EnableProgress::Continue);
                }
                _ => {
                    self.enable_step = None;
                    return Err(JoyConError::JoyConReportError(
                        crate::result::JoyConReportError::EmptyReport,
                    ));
                }
            }
        }
        Ok(EnableProgress::Continue)
    }

    pub fn enable<D: JoyConDriver>(&mut self, driver: &mut D) -> JoyConResult<()> {
        self.begin_enable();
        let overall = Instant::now() + MCU_WAIT_BUDGET * 4;
        while Instant::now() < overall {
            match self.advance_enable(driver, ENABLE_STEP_BUDGET)? {
                EnableProgress::Done => return Ok(()),
                EnableProgress::Continue => {}
            }
        }
        self.enable_step = None;
        Err(JoyConError::JoyConReportError(
            crate::result::JoyConReportError::EmptyReport,
        ))
    }

    pub fn disable<D: JoyConDriver>(&mut self, driver: &mut D) -> JoyConResult<()> {
        self.enable_step = None;
        if !self.active {
            return Ok(());
        }
        let _ = send_nfc_request(driver, NFC_STOP_PAYLOAD);
        self.active = false;
        self.last_state = 0;
        self.tag_present_last = false;
        self.removal_idle_streak = 0;
        self.error_streak = 0;
        set_report_mode(driver, InputReportId::StandardFull)?;
        call_subcmd_bytes(driver, SubCommand::Set_NFC_IR_MCUState as u8, &mcu_mode_bytes(MCUMode::Suspend))?;
        log::info!("[joycon-rs] NFC session disabled");
        Ok(())
    }

    pub fn tick_poll<D: JoyConDriver>(&self, driver: &mut D) -> JoyConResult<()> {
        if !self.active {
            return Ok(());
        }
        poll_mcu_stream(driver, MCU_STREAM_NFC)?;
        send_nfc_request(driver, NFC_POLL_PAYLOAD)
    }

    /// Force a new tag discovery cycle (e.g. after tag removal or when stuck on old UID).
    pub fn restart_scan<D: JoyConDriver>(&mut self, driver: &mut D) -> JoyConResult<()> {
        if !self.active {
            return Ok(());
        }
        self.recover_from_error(driver)?;
        log::info!("[joycon-rs] NFC scan restarted for new tag");
        Ok(())
    }

    /// Re-bootstrap NFC after MCU/NFC-IC error states (0x07 etc.).
    fn recover_from_error<D: JoyConDriver>(&mut self, driver: &mut D) -> JoyConResult<()> {
        if self
            .last_error_recover
            .is_some_and(|t| t.elapsed() < ERROR_RECOVERY_COOLDOWN)
        {
            return Ok(());
        }
        let _ = send_nfc_request(driver, NFC_CANCEL_PAYLOAD);
        let _ = send_nfc_request(driver, NFC_STOP_PAYLOAD);
        thread::sleep(Duration::from_millis(ERROR_RECOVERY_SLEEP_MS));
        let _ = send_mcu_cmd(driver, MCUCommand::set_mcu_mode(MCUMode::NFC));
        poll_mcu_stream(driver, MCU_STREAM_NFC)?;
        send_nfc_request(driver, NFC_DISCOVERY_PAYLOAD)?;
        send_nfc_request(driver, NFC_POLL_PAYLOAD)?;
        self.last_state = 0x01;
        self.tag_present_last = false;
        self.removal_idle_streak = 0;
        self.error_streak = 0;
        self.last_error_recover = Some(Instant::now());
        log::info!("[joycon-rs] NFC error recovery complete");
        Ok(())
    }

    /// Parse NFC state from a 0x31 report without updating session state.
    pub fn parse_from_raw(buf: &[u8; 362]) -> Option<NfcSample> {
        if buf[0] != InputReportId::StandardFullMCU as u8 {
            return None;
        }
        let mut report = InputReport::new();
        report.as_bytes_mut().copy_from_slice(buf);
        report.mcu_report().and_then(parse_nfc_state)
    }

    /// Apply one coalesced sample per listener tick (handles removal debounce + restart).
    pub fn apply_sample<D: JoyConDriver>(
        &mut self,
        driver: &mut D,
        sample: NfcSample,
    ) -> JoyConResult<NfcSample> {
        if !self.active {
            return Ok(sample);
        }

        if sample.tag_present {
            self.removal_idle_streak = 0;
            self.error_streak = 0;
            self.tag_present_last = true;
            self.last_state = sample.state;
            return Ok(sample);
        }

        if nfc_state_error(sample.state) {
            self.error_streak = self.error_streak.saturating_add(1);
            if self.error_streak >= ERROR_RECOVERY_STREAK {
                log::warn!(
                    "[joycon-rs] NFC IC error state=0x{:02x} (streak={}), recovering",
                    sample.state,
                    self.error_streak
                );
                self.recover_from_error(driver)?;
                return Ok(NfcSample {
                    state: 0x01,
                    tag_present: false,
                    tag_detected: false,
                    tag_type: 0,
                    uid: Vec::new(),
                });
            }
        } else {
            self.error_streak = 0;
        }

        // After a successful read, return to polling so the next tag can be discovered.
        if self.tag_present_last && nfc_state_error(sample.state) {
            self.recover_from_error(driver)?;
            self.tag_present_last = false;
            return Ok(NfcSample {
                state: 0x01,
                tag_present: false,
                tag_detected: false,
                tag_type: 0,
                uid: Vec::new(),
            });
        }

        let was_present = self.tag_present_last;
        if was_present && nfc_state_ready(sample.state) {
            self.removal_idle_streak = self.removal_idle_streak.saturating_add(1);
            if self.removal_idle_streak >= REMOVAL_IDLE_STREAK {
                restart_polling(driver)?;
                self.removal_idle_streak = 0;
                self.tag_present_last = false;
                self.last_state = 0x01;
                return Ok(NfcSample {
                    state: 0x01,
                    tag_present: false,
                    tag_detected: false,
                    tag_type: 0,
                    uid: Vec::new(),
                });
            }
            self.last_state = sample.state;
            return Ok(NfcSample {
                state: sample.state,
                tag_present: false,
                tag_detected: false,
                tag_type: sample.tag_type,
                uid: Vec::new(),
            });
        }

        self.removal_idle_streak = 0;
        if !was_present {
            self.tag_present_last = false;
        }
        self.last_state = sample.state;
        Ok(NfcSample {
            tag_type: sample.tag_type,
            ..sample
        })
    }

    pub fn process_raw_report<D: JoyConDriver>(
        &mut self,
        driver: &mut D,
        buf: &[u8; 362],
    ) -> JoyConResult<Option<NfcSample>> {
        if !self.active {
            return Ok(None);
        }
        let Some(sample) = Self::parse_from_raw(buf) else {
            return Ok(None);
        };
        self.apply_sample(driver, sample).map(Some)
    }
}

impl Default for NfcSession {
    fn default() -> Self {
        Self::new()
    }
}

/// HID + MCU header peek for diagnostics (full `0x31` report byte offsets).
pub fn peek_nfc_report(buf: &[u8; 362]) -> (u8, u8, u8, u8, u8) {
    let hid_id = buf[0];
    if hid_id != InputReportId::StandardFullMCU as u8 {
        return (hid_id, 0, 0, 0, 0);
    }
    let mcu_id = buf[49];
    // CTCaer layout relative to byte 49: state@56, has_tag@60, uid_len@64
    let state = buf[56];
    let has_tag = buf[60];
    let uid_len = buf[64];
    (hid_id, mcu_id, state, has_tag, uid_len)
}

pub fn parse_nfc_state(mcu: &MCUReport) -> Option<NfcSample> {
    if mcu.id() != MCUReportId::NFCState {
        return None;
    }
    let raw = mcu.as_bytes();
    if raw.len() < 17 {
        return None;
    }
    // CTCaer layout (MCU report byte offsets): state@7, has_tag@11, tag_ic@13, uid_len@15, uid@16
    let state = raw[7];
    let has_tag_info = raw[11];
    let tag_type = raw[13];
    let uid_len = raw[15];
    let uid_start = 16usize;
    let end = uid_start.saturating_add(uid_len as usize);
    let has_valid_tag_data = has_tag_info == 1 && uid_len > 0 && end <= raw.len();
    let uid = if has_valid_tag_data {
        raw[uid_start..end].to_vec()
    } else {
        Vec::new()
    };
    // CTCaer NFC IC states: 0x02 reading, 0x04 read finished, 0x09 detected.
    let tag_present = has_valid_tag_data && matches!(state, 0x04 | 0x09);
    let tag_detected =
        has_valid_tag_data && matches!(state, 0x02 | 0x03 | 0x05 | 0x06);
    Some(NfcSample {
        state,
        tag_present,
        tag_detected,
        tag_type: if has_valid_tag_data { tag_type } else { 0 },
        uid: if tag_present || tag_detected {
            uid
        } else {
            Vec::new()
        },
    })
}

/// Merge multiple NFC samples from one listener tick (main read + drain reads).
/// Prefers a confirmed tag; otherwise keeps the most active non-idle state so a
/// trailing idle poll does not mask an in-progress read.
pub fn coalesce_nfc_samples(samples: &[NfcSample]) -> Option<NfcSample> {
    if samples.is_empty() {
        return None;
    }
    if let Some(s) = samples.iter().find(|s| s.tag_present) {
        return Some(s.clone());
    }
    if let Some(s) = samples.iter().find(|s| s.tag_detected) {
        return Some(s.clone());
    }
    if let Some(s) = samples.iter().find(|s| nfc_state_ready(s.state)) {
        return Some(s.clone());
    }
    samples
        .iter()
        .filter(|s| !nfc_state_error(s.state) || s.tag_present || s.tag_detected)
        .max_by_key(|s| s.state as u16)
        .cloned()
        .or_else(|| samples.last().cloned())
}

/// Returns true when the previous tick had a tag and the MCU now reports idle polling.
pub fn should_restart_after_tag_removal(was_present: bool, sample: &NfcSample) -> bool {
    was_present && !sample.tag_present && nfc_state_ready(sample.state)
}

/// Joy-Con NFC state: idle / polling — safe to present a new tag.
pub fn nfc_state_ready(state: u8) -> bool {
    matches!(state, 0x00 | 0x01)
}

/// NFC IC error / reset-required states (CTCaer).
pub fn nfc_state_error(state: u8) -> bool {
    matches!(state, 0x07 | 0x0D | 0x0E)
}

/// Joy-Con NFC state: actively reading a recognized tag (not idle poll noise).
pub fn nfc_state_scanning(tag_detected: bool, tag_present: bool) -> bool {
    tag_detected && !tag_present
}

fn is_empty_read(err: &JoyConError) -> bool {
    matches!(
        err,
        JoyConError::JoyConReportError(crate::result::JoyConReportError::EmptyReport)
    )
}

fn mcu_cmd_bytes(cmd: MCUCommand) -> Vec<u8> {
    unsafe {
        std::slice::from_raw_parts(
            &cmd as *const MCUCommand as *const u8,
            std::mem::size_of::<MCUCommand>(),
        )
        .to_vec()
    }
}

fn mcu_mode_bytes(mode: MCUMode) -> Vec<u8> {
    vec![mode as u8]
}

fn call_subcmd_bytes<D: JoyConDriver>(driver: &mut D, subcmd: u8, data: &[u8]) -> JoyConResult<()> {
    match driver.send_sub_command_raw(subcmd, data)? {
        SubCommandReply::Checked(_) | SubCommandReply::Unchecked => Ok(()),
    }
}

fn set_report_mode<D: JoyConDriver>(driver: &mut D, mode: InputReportId) -> JoyConResult<()> {
    call_subcmd_bytes(driver, SubCommand::SetInputReportMode as u8, &[mode as u8])
}

fn send_mcu_cmd<D: JoyConDriver>(driver: &mut D, cmd: MCUCommand) -> JoyConResult<()> {
    call_subcmd_bytes(
        driver,
        SubCommand::Set_NFC_IR_MCUConfiguration as u8,
        &mcu_cmd_bytes(cmd),
    )
}

fn wait_mcu_status_until<D: JoyConDriver>(
    driver: &mut D,
    mode: MCUMode,
    deadline: Instant,
) -> JoyConResult<bool> {
    wait_mcu_cond_until(
        driver,
        MCURequestEnum::GetMCUStatus(()),
        deadline,
        |r| {
            r.state_report()
                .map(|s| s.state == mode)
                .unwrap_or(false)
        },
    )
}

fn wait_nfc_ready_until<D: JoyConDriver>(
    driver: &mut D,
    deadline: Instant,
) -> JoyConResult<bool> {
    while Instant::now() < deadline {
        send_nfc_request(driver, NFC_POLL_PAYLOAD)?;
        for _ in 0..6 {
            if Instant::now() >= deadline {
                return Ok(false);
            }
            let report = match read_input(driver) {
                Ok(r) => r,
                Err(e) if is_empty_read(&e) => continue,
                Err(e) => return Err(e),
            };
            if let Some(mcu) = report.mcu_report() {
                if mcu.is_busy_init() {
                    continue;
                }
                if mcu.id() == MCUReportId::NFCState {
                    let raw = mcu.as_bytes();
                    if raw.len() > 7 && (raw[7] == 0x01 || raw[7] == 0x09) {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

fn wait_mcu_cond_until<D: JoyConDriver>(
    driver: &mut D,
    request: impl Into<MCURequest> + Copy,
    deadline: Instant,
    mut pred: impl FnMut(&MCUReport) -> bool,
) -> JoyConResult<bool> {
    let request = request.into();
    while Instant::now() < deadline {
        write_output(driver, request.into())?;
        for _ in 0..6 {
            if Instant::now() >= deadline {
                return Ok(false);
            }
            let report = match read_input(driver) {
                Ok(r) => r,
                Err(e) if is_empty_read(&e) => continue,
                Err(e) => return Err(e),
            };
            if let Some(mcu) = report.mcu_report() {
                if mcu.is_busy_init() {
                    continue;
                }
                if pred(mcu) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn send_nfc_request<D: JoyConDriver>(driver: &mut D, payload: &[u8]) -> JoyConResult<()> {
    write_output(driver, MCURequest::nfc_data(payload).into())
}

fn poll_mcu_stream<D: JoyConDriver>(driver: &mut D, mode: u8) -> JoyConResult<()> {
    write_output(driver, MCURequest::poll_stream(mode).into())
}

fn restart_polling<D: JoyConDriver>(driver: &mut D) -> JoyConResult<()> {
    let _ = send_nfc_request(driver, NFC_CANCEL_PAYLOAD);
    let _ = send_nfc_request(driver, NFC_STOP_PAYLOAD);
    thread::sleep(Duration::from_millis(40));
    send_nfc_request(driver, NFC_DISCOVERY_PAYLOAD)?;
    send_nfc_request(driver, NFC_POLL_PAYLOAD)
}

fn read_input<D: JoyConDriver>(driver: &D) -> JoyConResult<InputReport> {
    let mut buf = [0u8; 362];
    let n = driver.read_timeout(&mut buf, READ_TIMEOUT_MS)?;
    if n < 12 {
        return Err(JoyConError::JoyConReportError(
            crate::result::JoyConReportError::EmptyReport,
        ));
    }
    let mut report = InputReport::new();
    let out = report.as_bytes_mut();
    let copy = n.min(out.len());
    out[..copy].copy_from_slice(&buf[..copy]);
    Ok(report)
}

fn write_output<D: JoyConDriver>(driver: &mut D, mut report: OutputReport) -> JoyConResult<()> {
    *report.packet_counter() = driver.global_packet_number();
    driver.increase_global_packet_number();
    let n = driver.write(report.as_bytes())?;
    if n == 0 {
        return Err(JoyConError::JoyConReportError(
            crate::result::JoyConReportError::EmptyReport,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use joycon_sys::mcu::MCUReportId;

    fn nfc_mcu_report(state: u8, tag_type: u8, uid: &[u8]) -> MCUReport {
        let mut mcu = MCUReport::new();
        unsafe {
            let raw = &mut *(std::ptr::addr_of_mut!(mcu) as *mut [u8; 312]);
            raw[0] = MCUReportId::NFCState as u8;
            raw[7] = state;
            raw[11] = if uid.is_empty() { 0 } else { 1 };
            raw[13] = tag_type;
            raw[15] = uid.len() as u8;
            let end = 16usize.saturating_add(uid.len());
            raw[16..end].copy_from_slice(uid);
        }
        mcu
    }

    #[test]
    fn uid_hex_formats_bytes() {
        let sample = NfcSample {
            uid: vec![0x04, 0xab, 0xcd],
            ..Default::default()
        };
        assert_eq!(sample.uid_hex(), "04abcd");
    }

    #[test]
    fn parse_polling_state() {
        let mcu = nfc_mcu_report(0x01, 0x02, &[]);
        let sample = parse_nfc_state(&mcu).expect("nfc state");
        assert_eq!(sample.state, 0x01);
        assert!(!sample.tag_present);
        assert!(!sample.tag_detected);
        assert!(sample.uid.is_empty());
    }

    #[test]
    fn parse_tag_present_with_uid() {
        let uid = [0x04, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc];
        let mcu = nfc_mcu_report(0x09, 0x02, &uid);
        let sample = parse_nfc_state(&mcu).expect("nfc state");
        assert!(sample.tag_present);
        assert_eq!(sample.state, 0x09);
        assert_eq!(sample.tag_type, 0x02);
        assert_eq!(sample.uid, uid.to_vec());
        assert_eq!(sample.uid_hex(), "04123456789abc");
    }

    #[test]
    fn parse_polling_clears_stale_uid() {
        let uid = [0x04, 0xab, 0xcd];
        let mcu = nfc_mcu_report(0x01, 0x02, &uid);
        let sample = parse_nfc_state(&mcu).expect("nfc state");
        assert!(!sample.tag_present);
        assert!(sample.uid.is_empty());
    }

    #[test]
    fn parse_tag_via_has_tag_info_does_not_mark_present_without_state_09() {
        let uid = [0x04, 0xab, 0xcd];
        let mcu = nfc_mcu_report(0x01, 0x02, &uid);
        let sample = parse_nfc_state(&mcu).expect("nfc state");
        assert!(!sample.tag_present);
        assert!(sample.uid.is_empty());
    }

    #[test]
    fn parse_state_09_without_tag_info_is_not_present() {
        let uid = [0x04, 0xab, 0xcd];
        let mut mcu = nfc_mcu_report(0x09, 0x02, &uid);
        unsafe {
            let raw = &mut *(std::ptr::addr_of_mut!(mcu) as *mut [u8; 312]);
            raw[11] = 0;
        }
        let sample = parse_nfc_state(&mcu).expect("nfc state");
        assert!(!sample.tag_present);
        assert!(sample.uid.is_empty());
    }

    #[test]
    fn parse_ignores_non_nfc_reports() {
        let mut mcu = MCUReport::new();
        unsafe {
            let raw = &mut *(std::ptr::addr_of_mut!(mcu) as *mut [u8; 312]);
            raw[0] = MCUReportId::IRData as u8;
        }
        assert!(parse_nfc_state(&mcu).is_none());
    }

    #[test]
    fn parse_error_state_with_uid_is_not_present() {
        let uid = [0x57, 0x41, 0xbc, 0x5e];
        let mcu = nfc_mcu_report(0x07, 0x90, &uid);
        let sample = parse_nfc_state(&mcu).expect("nfc state");
        assert!(!sample.tag_present);
        assert!(!sample.tag_detected);
        assert!(sample.uid.is_empty());
        assert!(nfc_state_error(0x07));
    }

    #[test]
    fn parse_read_finished_marks_present() {
        let uid = [0x04, 0xab, 0xcd];
        let mcu = nfc_mcu_report(0x04, 0x02, &uid);
        let sample = parse_nfc_state(&mcu).expect("nfc state");
        assert!(sample.tag_present);
        assert_eq!(sample.uid, uid.to_vec());
    }

    #[test]
    fn nfc_state_ready_and_scanning() {
        assert!(nfc_state_ready(0x00));
        assert!(nfc_state_ready(0x01));
        assert!(!nfc_state_ready(0x09));
        assert!(nfc_state_scanning(true, false));
        assert!(!nfc_state_scanning(false, false));
        assert!(!nfc_state_scanning(true, true));
    }

    #[test]
    fn parse_tag_detected_mid_read() {
        let uid = [0x04, 0xab, 0xcd];
        let mcu = nfc_mcu_report(0x05, 0x02, &uid);
        let sample = parse_nfc_state(&mcu).expect("nfc state");
        assert!(!sample.tag_present);
        assert!(sample.tag_detected);
        assert_eq!(sample.uid, uid.to_vec());
    }

    #[test]
    fn parse_spurious_state_without_tag_info_is_not_scanning() {
        let mcu = nfc_mcu_report(0x05, 0x02, &[]);
        let sample = parse_nfc_state(&mcu).expect("nfc state");
        assert!(!sample.tag_present);
        assert!(!sample.tag_detected);
    }

    #[test]
    fn removal_restart_only_on_idle_polling() {
        let uid = [0x04, 0xab, 0xcd];
        let present = parse_nfc_state(&nfc_mcu_report(0x09, 0x02, &uid)).unwrap();
        assert!(present.tag_present);

        let scanning = parse_nfc_state(&nfc_mcu_report(0x05, 0x02, &[])).unwrap();
        assert!(!should_restart_after_tag_removal(true, &scanning));

        let idle = parse_nfc_state(&nfc_mcu_report(0x01, 0x02, &[])).unwrap();
        assert!(should_restart_after_tag_removal(true, &idle));
        assert!(!should_restart_after_tag_removal(false, &idle));
    }

    #[test]
    fn coalesce_prefers_tag_present_over_trailing_idle() {
        let uid = [0x04, 0xab, 0xcd];
        let present = parse_nfc_state(&nfc_mcu_report(0x09, 0x02, &uid)).unwrap();
        let idle = parse_nfc_state(&nfc_mcu_report(0x01, 0x02, &[])).unwrap();
        let merged = coalesce_nfc_samples(&[idle.clone(), present.clone()]).unwrap();
        assert!(merged.tag_present);
        assert_eq!(merged.uid, present.uid);
    }

    #[test]
    fn coalesce_prefers_idle_over_spurious_scanning() {
        let scanning = parse_nfc_state(&nfc_mcu_report(0x05, 0x02, &[])).unwrap();
        let idle = parse_nfc_state(&nfc_mcu_report(0x01, 0x02, &[])).unwrap();
        let merged = coalesce_nfc_samples(&[idle.clone(), scanning]).unwrap();
        assert_eq!(merged.state, 0x01);
        assert!(!merged.tag_detected);
    }

    #[test]
    fn coalesce_keeps_tag_detected_over_idle() {
        let uid = [0x04, 0xab, 0xcd];
        let reading = parse_nfc_state(&nfc_mcu_report(0x05, 0x02, &uid)).unwrap();
        let idle = parse_nfc_state(&nfc_mcu_report(0x01, 0x02, &[])).unwrap();
        let merged = coalesce_nfc_samples(&[idle, reading.clone()]).unwrap();
        assert!(merged.tag_detected);
        assert_eq!(merged.state, 0x05);
    }
}
