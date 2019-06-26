//! Low-level enums and functions related to the CANifier.
#![allow(non_camel_case_types)]

use std::os::raw;

use super::ErrorCode;

#[doc(hidden)]
#[repr(C)]
pub struct Device {
    _private: [u8; 0],
}
/// A handle representing a CANifier.
pub type Handle = *mut Device;

/* automatically generated by rust-bindgen */

#[repr(i32)]
/// Enumerated type for status frame types.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CANifierControlFrame {
    Control_1_General = 0x03040000,
    Control_2_PwmOutput = 0x03040040,
}
#[repr(i32)]
/// Enumerated type for status frame types.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CANifierStatusFrame {
    Status_1_General = 0x041400,
    Status_2_General = 0x041440,
    Status_3_PwmInputs0 = 0x041480,
    Status_4_PwmInputs1 = 0x0414C0,
    Status_5_PwmInputs2 = 0x041500,
    Status_6_PwmInputs3 = 0x041540,
    Status_8_Misc = 0x0415C0,
}

#[repr(u32)]
/// General IO Pins on the CANifier
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum GeneralPin {
    QUAD_IDX = 0,
    QUAD_B = 1,
    QUAD_A = 2,
    LIMR = 3,
    LIMF = 4,
    SDA = 5,
    SCL = 6,
    SPI_CS = 7,
    SPI_MISO_PWM2P = 8,
    SPI_MOSI_PWM1P = 9,
    SPI_CLK_PWM0P = 10,
}

extern "C" {
    pub fn c_CANifier_Create1(deviceNumber: raw::c_int) -> Handle;

    pub fn c_CANifier_GetDescription(
        handle: Handle,
        toFill: *mut raw::c_char,
        toFillByteSz: raw::c_int,
        numBytesFilled: *mut raw::c_int,
    ) -> ErrorCode;

    pub fn c_CANifier_SetLEDOutput(handle: Handle, dutyCycle: u32, ledChannel: u32) -> ErrorCode;

    pub fn c_CANifier_SetGeneralOutputs(
        handle: Handle,
        outputsBits: u32,
        isOutputBits: u32,
    ) -> ErrorCode;

    pub fn c_CANifier_SetGeneralOutput(
        handle: Handle,
        outputPin: u32,
        outputValue: bool,
        outputEnable: bool,
    ) -> ErrorCode;

    pub fn c_CANifier_SetPWMOutput(handle: Handle, pwmChannel: u32, dutyCycle: u32) -> ErrorCode;

    pub fn c_CANifier_EnablePWMOutput(handle: Handle, pwmChannel: u32, bEnable: bool) -> ErrorCode;

    pub fn c_CANifier_GetGeneralInputs(
        handle: Handle,
        allPins: *mut bool,
        capacity: u32,
    ) -> ErrorCode;

    pub fn c_CANifier_GetGeneralInput(
        handle: Handle,
        inputPin: u32,
        measuredInput: *mut bool,
    ) -> ErrorCode;

    pub fn c_CANifier_GetPWMInput(
        handle: Handle,
        pwmChannel: u32,
        pulseWidthAndPeriod: &mut [f64; 2],
    ) -> ErrorCode;

    pub fn c_CANifier_GetLastError(handle: Handle) -> ErrorCode;

    pub fn c_CANifier_GetBusVoltage(handle: Handle, batteryVoltage: *mut f64) -> ErrorCode;

    pub fn c_CANifier_GetQuadraturePosition(handle: Handle, pos: *mut raw::c_int) -> ErrorCode;

    pub fn c_CANifier_SetQuadraturePosition(
        handle: Handle,
        pos: raw::c_int,
        timeoutMs: raw::c_int,
    ) -> ErrorCode;

    pub fn c_CANifier_GetQuadratureVelocity(handle: Handle, vel: *mut raw::c_int) -> ErrorCode;

    pub fn c_CANifier_GetQuadratureSensor(
        handle: Handle,
        pos: *mut raw::c_int,
        vel: *mut raw::c_int,
    ) -> ErrorCode;

    pub fn c_CANifier_ConfigVelocityMeasurementPeriod(
        handle: Handle,
        period: raw::c_int,
        timeoutMs: raw::c_int,
    ) -> ErrorCode;

    pub fn c_CANifier_ConfigVelocityMeasurementWindow(
        handle: Handle,
        window: raw::c_int,
        timeoutMs: raw::c_int,
    ) -> ErrorCode;

    pub fn c_CANifier_SetLastError(handle: Handle, error: raw::c_int);

    pub fn c_CANifier_ConfigSetParameter(
        handle: Handle,
        param: raw::c_int,
        value: f64,
        subValue: raw::c_int,
        ordinal: raw::c_int,
        timeoutMs: raw::c_int,
    ) -> ErrorCode;

    pub fn c_CANifier_ConfigGetParameter(
        handle: Handle,
        param: raw::c_int,
        value: *mut f64,
        ordinal: raw::c_int,
        timeoutMs: raw::c_int,
    ) -> ErrorCode;

    pub fn c_CANifier_ConfigSetCustomParam(
        handle: Handle,
        newValue: raw::c_int,
        paramIndex: raw::c_int,
        timeoutMs: raw::c_int,
    ) -> ErrorCode;

    pub fn c_CANifier_ConfigGetCustomParam(
        handle: Handle,
        readValue: *mut raw::c_int,
        paramIndex: raw::c_int,
        timoutMs: raw::c_int,
    ) -> ErrorCode;

    pub fn c_CANifier_GetFaults(handle: Handle, param: *mut raw::c_int) -> ErrorCode;

    pub fn c_CANifier_GetStickyFaults(handle: Handle, param: *mut raw::c_int) -> ErrorCode;

    pub fn c_CANifier_ClearStickyFaults(handle: Handle, timeoutMs: raw::c_int) -> ErrorCode;

    pub fn c_CANifier_GetFirmwareVersion(
        handle: Handle,
        firmwareVers: *mut raw::c_int,
    ) -> ErrorCode;

    pub fn c_CANifier_HasResetOccurred(handle: Handle, hasReset: *mut bool) -> ErrorCode;

    pub fn c_CANifier_SetStatusFramePeriod(
        handle: Handle,
        frame: raw::c_int,
        periodMs: raw::c_int,
        timeoutMs: raw::c_int,
    ) -> ErrorCode;

    pub fn c_CANifier_GetStatusFramePeriod(
        handle: Handle,
        frame: raw::c_int,
        periodMs: *mut raw::c_int,
        timeoutMs: raw::c_int,
    ) -> ErrorCode;

    pub fn c_CANifier_SetControlFramePeriod(
        handle: Handle,
        frame: raw::c_int,
        periodMs: raw::c_int,
    ) -> ErrorCode;
}
