//! IR camera session for Joy-Con (R) — ported from [Yamakaky/joy](https://github.com/Yamakaky/joy) (MIT).
//!
//! Gated behind the `ir` feature; inactive unless explicitly enabled by the caller.

use crate::joycon::driver::{JoyConDriver, SubCommand, SubCommandReply};
use crate::result::{JoyConError, JoyConResult};
use joycon_sys::common::InputReportId;
use joycon_sys::mcu::ir::{IRRequest, IRRequestEnum, MCUIRMode};
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

fn is_empty_read(err: &JoyConError) -> bool {
    matches!(
        err,
        JoyConError::JoyConReportError(crate::result::JoyConReportError::EmptyReport)
    )
}

/// Latest IR sensor sample (PulseRate / clustering summary fields).
#[derive(Debug, Clone, Copy, Default)]
pub struct IrSample {
    pub average_intensity: u8,
    pub white_pixel_count: u16,
    pub ambient_noise_count: u16,
}

impl IrSample {
    /// Heuristic: hand/object close to the IR camera.
    pub fn proximity_detected(&self, white_pixel_threshold: u16) -> bool {
        self.white_pixel_count > white_pixel_threshold
    }
}

/// Active IR session on a right Joy-Con. Switches the controller to report mode `0x31`.
pub struct IrSession {
    active: bool,
    enable_step: Option<u8>,
    enable_fw: (joycon_sys::common::U16LE, joycon_sys::common::U16LE),
    last_frag: u8,
}

impl IrSession {
    pub fn new() -> Self {
        Self {
            active: false,
            enable_step: None,
            enable_fw: (joycon_sys::common::U16LE::from(0), joycon_sys::common::U16LE::from(0)),
            last_frag: 0,
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
                    self.set_report_mode(driver, InputReportId::StandardFullMCU)?;
                    self.enable_step = Some(1);
                    step = 1;
                }
                1 => {
                    let bytes = mcu_mode_bytes(MCUMode::Standby);
                    self.call_subcmd_bytes(driver, SubCommand::Set_NFC_IR_MCUState as u8, &bytes)?;
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
                    self.send_mcu_cmd(driver, MCUCommand::set_mcu_mode(MCUMode::IR))?;
                    self.enable_step = Some(4);
                    step = 4;
                }
                4 => {
                    if wait_mcu_status_until(driver, MCUMode::IR, deadline)? {
                        self.enable_step = Some(5);
                        step = 5;
                    } else {
                        return Ok(EnableProgress::Continue);
                    }
                }
                5 => {
                    if wait_mcu_cond_until(
                        driver,
                        MCURequestEnum::GetMCUStatus(()),
                        deadline,
                        |r| {
                            if let Some(status) = r.state_report() {
                                self.enable_fw =
                                    (status.fw_major_version, status.fw_minor_version);
                                true
                            } else {
                                false
                            }
                        },
                    )? {
                        self.enable_step = Some(6);
                        step = 6;
                    } else {
                        return Ok(EnableProgress::Continue);
                    }
                }
                6 => {
                    let cmd = MCUCommand::configure_ir_ir(joycon_sys::mcu::ir::MCUIRModeData {
                        ir_mode: MCUIRMode::PulseRate.into(),
                        no_of_frags: 1,
                        mcu_fw_version: self.enable_fw,
                    });
                    self.send_mcu_cmd(driver, cmd)?;
                    self.enable_step = Some(7);
                    step = 7;
                }
                7 => {
                    if wait_mcu_cond_until(
                        driver,
                        IRRequestEnum::GetState(()),
                        deadline,
                        |r| {
                            r.ir_status()
                                .map(|s| s.ir_mode == MCUIRMode::PulseRate)
                                .unwrap_or(false)
                        },
                    )? {
                        self.enable_step = Some(8);
                        step = 8;
                    } else {
                        return Ok(EnableProgress::Continue);
                    }
                }
                8 => {
                    let tuning = [3u8, 0, 0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
                        255, 255, 255, 255, 9];
                    self.call_subcmd_bytes(driver, 0x24, &tuning)?;
                    self.active = true;
                    self.enable_step = None;
                    log::info!("[joycon-rs] IR PulseRate session enabled");
                    return Ok(EnableProgress::Done);
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

    /// Enable PulseRate mode — lightweight proximity / motion over IR (no full image transfer).
    pub fn enable_pulse_rate<D: JoyConDriver>(&mut self, driver: &mut D) -> JoyConResult<()> {
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

    /// Restore standard `0x30` input and suspend the NFC/IR MCU.
    pub fn disable<D: JoyConDriver>(&mut self, driver: &mut D) -> JoyConResult<()> {
        self.enable_step = None;
        if !self.active {
            return Ok(());
        }
        self.active = false;
        self.set_report_mode(driver, InputReportId::StandardFull)?;
        let bytes = mcu_mode_bytes(MCUMode::Suspend);
        self.call_subcmd_bytes(driver, SubCommand::Set_NFC_IR_MCUState as u8, &bytes)?;
        log::info!("[joycon-rs] IR session disabled");
        Ok(())
    }

    /// PulseRate mode pushes IR data automatically once configured.
    pub fn tick_stream<D: JoyConDriver>(&self, _driver: &mut D) -> JoyConResult<()> {
        Ok(())
    }

    /// Process MCU data from a raw `0x31` input report (call from the main input loop).
    pub fn process_raw_report<D: JoyConDriver>(
        &mut self,
        driver: &mut D,
        buf: &[u8; 362],
    ) -> JoyConResult<Option<IrSample>> {
        if !self.active || buf[0] != InputReportId::StandardFullMCU as u8 {
            return Ok(None);
        }
        let mut report = InputReport::new();
        report.as_bytes_mut().copy_from_slice(buf);
        if let Some(mcu) = report.mcu_report() {
            return self.handle_mcu(driver, mcu);
        }
        Ok(None)
    }

    fn handle_mcu<D: JoyConDriver>(
        &mut self,
        driver: &mut D,
        mcu: &MCUReport,
    ) -> JoyConResult<Option<IrSample>> {
        if let Some(packet) = mcu.ir_data() {
            let ack = joycon_sys::mcu::ir::IRAckRequestPacket {
                packet_missing: joycon_sys::common::Bool::False.into(),
                missed_packet_id: 0,
                ack_packet_id: packet.frag_number,
            };
            let out: OutputReport = MCURequest::from(IRRequest::from(ack)).into();
            write_output(driver, out)?;

            self.last_frag = packet.frag_number;
            return Ok(Some(IrSample {
                average_intensity: packet.average_intensity,
                white_pixel_count: u16::from(packet.white_pixel_count),
                ambient_noise_count: u16::from(packet.ambient_noise_count),
            }));
        }
        if mcu.id() == MCUReportId::EmptyAwaitingCmd && self.last_frag > 0 {
            let out = OutputReport::ir_ack(self.last_frag);
            write_output(driver, out)?;
        }
        Ok(None)
    }

    fn set_report_mode<D: JoyConDriver>(
        &self,
        driver: &mut D,
        mode: InputReportId,
    ) -> JoyConResult<()> {
        self.call_subcmd_bytes(driver, SubCommand::SetInputReportMode as u8, &[mode as u8])
    }

    fn send_mcu_cmd<D: JoyConDriver>(&self, driver: &mut D, cmd: MCUCommand) -> JoyConResult<()> {
        let bytes = mcu_cmd_bytes(cmd);
        self.call_subcmd_bytes(driver, SubCommand::Set_NFC_IR_MCUConfiguration as u8, &bytes)
    }

    fn call_subcmd_bytes<D: JoyConDriver>(
        &self,
        driver: &mut D,
        subcmd: u8,
        data: &[u8],
    ) -> JoyConResult<()> {
        match driver.send_sub_command_raw(subcmd, data)? {
            SubCommandReply::Checked(_) | SubCommandReply::Unchecked => Ok(()),
        }
    }
}

impl Default for IrSession {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for IrSession {
    fn drop(&mut self) {
        // Cannot disable here without driver reference; caller must call disable().
    }
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

    #[test]
    fn proximity_detected_uses_white_pixel_threshold() {
        let sample = IrSample {
            white_pixel_count: 60,
            ..Default::default()
        };
        assert!(sample.proximity_detected(50));
        assert!(!sample.proximity_detected(80));
    }
}
