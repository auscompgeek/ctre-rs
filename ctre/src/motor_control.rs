//! Support for motor controllers (Talon SRX and Victor SPX).

use ctre_data::mot::config::*;
pub use ctre_data::mot::{
    api::*,
    cci::*,
    config::{BaseMotorControllerConfiguration, BasePIDSetConfiguration},
};
use ctre_sys::mot::*;
pub use ctre_sys::mot::{
    ControlFrame, ControlFrameEnhanced, ControlMode, InvertType, StatusFrame, StatusFrameEnhanced,
};
use memoffset::raw_field;
use std::mem;

use super::{motion::*, ConfigAll, CustomParam, ErrorCode, ParamEnum, Result};

/// The CTRE motor controller prelude.
///
/// The purpose of this module is to alleviate imports
/// of the most commonly used CTRE motor controller types and traits.
pub mod prelude {
    pub use super::MotorController as __ctre_MotorController;
    #[doc(no_inline)]
    pub use super::{ControlMode, Demand, TalonSRX, VictorSPX};
}

bitflags! {
    /// All the faults available to motor controllers
    pub struct Faults: i32 {
        /**
         * Motor Controller is under 6.5V
         */
        const UNDER_VOLTAGE = 1;
        /**
         * Forward limit switch is tripped and device is trying to go forward
         * Only trips when the device is limited
         */
        const FORWARD_LIMIT_SWITCH = 1 << 1;
        /**
         * Reverse limit switch is tripped and device is trying to go reverse
         * Only trips when the device is limited
         */
        const REVERSE_LIMIT_SWITCH = 1 << 2;
        /**
         * Sensor is beyond forward soft limit and device is trying to go forward
         * Only trips when the device is limited
         */
        const FORWARD_SOFT_LIMIT = 1 << 3;
        /**
         * Sensor is beyond reverse soft limit and device is trying to go reverse
         * Only trips when the device is limited
         */
        const REVERSE_SOFT_LIMIT = 1 << 4;
        /**
         * Device detects hardware failure
         */
        const HARDWARE_FAILURE = 1 << 5;
        /**
         * Device was powered-on or reset while robot is enabled.
         * Check your breakers and wiring.
         */
        const RESET_DURING_EN = 1 << 6;
        /**
         * Device's sensor overflowed
         */
        const SENSOR_OVERFLOW = 1 << 7;
        /**
         * Device detects its sensor is out of phase
         */
        const SENSOR_OUT_OF_PHASE = 1 << 8;
        /// Not used, see `RESET_DURING_EN`
        const HARDWARE_ESD_RESET = 1 << 9;
        /**
         * Remote Sensor is no longer detected on bus
         */
        const REMOTE_LOSS_OF_SIGNAL = 1 << 10;
        /**
         * API error detected.  Make sure API and firmware versions are compatible.
         */
        const API_ERROR = 1 << 11;
    }
}

bitflags! {
    /// All the faults available to motor controllers
    pub struct StickyFaults: i32 {
        /**
         * Motor Controller is under 6.5V
         */
        const UNDER_VOLTAGE = 1;
        /**
         * Forward limit switch is tripped and device is trying to go forward
         * Only trips when the device is limited
         */
        const FORWARD_LIMIT_SWITCH = 1 << 1;
        /**
         * Reverse limit switch is tripped and device is trying to go reverse
         * Only trips when the device is limited
         */
        const REVERSE_LIMIT_SWITCH = 1 << 2;
        /**
         * Sensor is beyond forward soft limit and device is trying to go forward
         * Only trips when the device is limited
         */
        const FORWARD_SOFT_LIMIT = 1 << 3;
        /**
         * Sensor is beyond reverse soft limit and device is trying to go reverse
         * Only trips when the device is limited
         */
        const REVERSE_SOFT_LIMIT = 1 << 4;
        /**
         * Device was powered-on or reset while robot is enabled.
         * Check your breakers and wiring.
         */
        const RESET_DURING_EN = 1 << 5;
        /**
         * Device's sensor overflowed
         */
        const SENSOR_OVERFLOW = 1 << 6;
        /**
         * Device detects its sensor is out of phase
         */
        const SENSOR_OUT_OF_PHASE = 1 << 7;
        /// Not used, see `RESET_DURING_EN`
        const HARDWARE_ESD_RESET = 1 << 8;
        /**
         * Remote Sensor is no longer detected on bus
         */
        const REMOTE_LOSS_OF_SIGNAL = 1 << 9;
        /**
         * Device detects an API error
         */
        const API_ERROR = 1 << 10;
    }
}

/// Which PID loop a config call should refer to.
/// Used in place of a `pidIdx` parameter in the C++/Java API.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum PIDLoop {
    /// The primary PID loop (PID index 0).
    Primary = 0,
    /// The auxiliary PID loop (PID index 1).
    Auxiliary = 1,
}

impl Default for PIDLoop {
    #[inline]
    /// Returns PIDLoop::Primary.
    fn default() -> Self {
        Self::Primary
    }
}

/// Remote sensor/filter ordinal.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum RemoteFilter {
    /// Remote Sensor 0
    S0 = 0,
    /// Remote Sensor 1
    S1 = 1,
}

/// Demand 1.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Demand {
    /// Apply no change to the demand0 output.
    Neutral,
    /**
     * When closed-looping, set the target of the auxiliary PID loop.
     *
     * When following, follow the processed output of the combined
     * primary/aux PID output.  The demand value is ignored.
     */
    AuxPID(f64),
    /// Simply add to the demand0 output.
    ///
    /// In PercentOutput the demand0 output is the motor output,
    /// and in closed-loop modes the demand0 output is the output of PID0.
    ArbitraryFeedForward(f64),
}

impl Default for Demand {
    #[inline]
    /// Returns Demand::Neutral.
    fn default() -> Self {
        Self::Neutral
    }
}

impl Demand {
    /// Get the type of this demand1.
    pub fn type_(&self) -> DemandType {
        match self {
            Demand::Neutral => DemandType::Neutral,
            Demand::AuxPID(_) => DemandType::AuxPID,
            Demand::ArbitraryFeedForward(_) => DemandType::ArbitraryFeedForward,
        }
    }

    /// Get the raw value of this demand1.
    pub fn value(self) -> f64 {
        match self {
            Demand::Neutral => 0.0,
            Demand::AuxPID(value) | Demand::ArbitraryFeedForward(value) => value,
        }
    }
}

/// This is split from the trait to reduce compiled size under monomorphization.
/// Caller must pack the 24-bit base arbitration ID for Follower mode.
fn _set(handle: Handle, mode: ControlMode, demand0: f64, demand1: Demand) {
    match mode {
        | ControlMode::PercentOutput
        //| ControlMode::TimedPercentOutput
        | ControlMode::Follower
        | ControlMode::Velocity
        | ControlMode::Position
        | ControlMode::MotionMagic
        //| ControlMode::MotionMagicArc
        | ControlMode::MotionProfile
        | ControlMode::MotionProfileArc => unsafe {
            c_MotController_Set_4(
                handle,
                mode as _,
                demand0,
                demand1.value(),
                demand1.type_() as _,
            )
        },
        ControlMode::Current => unsafe {
            // milliamps
            c_MotController_SetDemand(handle, mode as _, (1000.0 * demand0) as _, 0)
        },
        ControlMode::Disabled => unsafe {
            c_MotController_SetDemand(handle, mode as _, 0, 0)
        },
    };
}

/// Get motion profile status information.
/// This is split from the trait to reduce compiled size under monomorphization.
unsafe fn _motion_profile_status(
    handle: Handle,
    status_to_fill: *mut MotionProfileStatus,
) -> ErrorCode {
    let mut output_enable = 0;
    let mut profile_slot_select_0 = 0;
    let mut profile_slot_select_1 = 0;
    let code = c_MotController_GetMotionProfileStatus_2(
        handle,
        raw_field!(status_to_fill, MotionProfileStatus, top_buffer_rem) as *mut _,
        raw_field!(status_to_fill, MotionProfileStatus, top_buffer_cnt) as *mut _,
        raw_field!(status_to_fill, MotionProfileStatus, btm_buffer_cnt) as *mut _,
        raw_field!(status_to_fill, MotionProfileStatus, has_underrun) as *mut _,
        raw_field!(status_to_fill, MotionProfileStatus, is_underrun) as *mut _,
        raw_field!(status_to_fill, MotionProfileStatus, active_point_valid) as *mut _,
        raw_field!(status_to_fill, MotionProfileStatus, is_last) as *mut _,
        &mut profile_slot_select_0,
        &mut output_enable,
        raw_field!(status_to_fill, MotionProfileStatus, time_dur_ms) as *mut _,
        &mut profile_slot_select_1,
    );
    (*status_to_fill).output_enable = output_enable.into();
    (*status_to_fill).profile_slot_select_0 = profile_slot_select_0;
    (*status_to_fill).profile_slot_select_1 = profile_slot_select_1;
    code
}

/// Base motor controller features for all CTRE CAN motor controllers.
///
/// This trait is sealed and cannot be implemented for types outside this crate.
pub trait MotorController: private::Sealed {
    #[doc(hidden)]
    fn handle(&self) -> Handle;
    #[doc(hidden)]
    fn base_id(&self) -> i32;
    fn device_id(&self) -> i32 {
        cci_get_only!(c_MotController_GetDeviceNumber(self.handle(), _: i32))
    }

    /**
     * # Parameters
     *
     * - `mode`: Sets the appropriate output on the talon, depending on the mode.
     * - `demand0`: The output value to apply.
     *   such as advanced feed forward and/or auxiliary close-looping in firmware.
     *   * In PercentOutput, the output is between -1.0 and 1.0, with 0.0 as stopped.
     *   * In Current mode, output value is in amperes.
     *   * In Velocity mode, output value is in position change / 100ms.
     *   * In Position mode, output value is in encoder ticks or an analog value,
     *     depending on the sensor. See
     *   * In Follower mode, the output value is the integer device ID of the talon to duplicate.
     * - `demand1`: Supplmental output value.  Units match the set mode.
     *
     * # Examples
     *
     * Standard Driving Example:
     * ```
     * # use ctre::mot::prelude::*;
     * let mut talon_left = TalonSRX::new(0);
     * let mut talon_rght = TalonSRX::new(1);
     * # let left_joy = 0.5;
     * # let rght_joy = 0.5;
     * talon_left.set(ControlMode::PercentOutput, left_joy, Demand::Neutral);
     * talon_rght.set(ControlMode::PercentOutput, rght_joy, Demand::Neutral);
     * ```
     *
     * Arcade Drive Example:
     * ```
     * # use ctre::mot::prelude::*;
     * let mut talon_left = TalonSRX::new(0);
     * let mut talon_rght = TalonSRX::new(1);
     * # let joy_fwd = 0.5;
     * # let joy_turn = 0.1;
     * talon_left.set(ControlMode::PercentOutput, joy_fwd, Demand::ArbitraryFeedForward(joy_turn));
     * talon_rght.set(ControlMode::PercentOutput, joy_fwd, Demand::ArbitraryFeedForward(-joy_turn));
     * ```
     *
     * Drive Straight Example:
     * Note: Selected Sensor Configuration is necessary for both PID0 and PID1.
     * ```
     * # use ctre::mot::{prelude::*, FollowerType};
     * let mut talon_left = TalonSRX::new(0);
     * let mut talon_rght = TalonSRX::new(1);
     * # let joy_fwd = 0.5;
     * # let desired_robot_heading = 0.0;
     * talon_left.follow(&talon_rght, FollowerType::AuxOutput1);
     * talon_rght.set(ControlMode::PercentOutput, joy_fwd, Demand::AuxPID(desired_robot_heading));
     * ```
     *
     * Drive Straight to a Distance Example:
     * Note: Other configurations (sensor selection, PID gains, etc.) need to be set.
     * ```
     * # use ctre::mot::{prelude::*, FollowerType};
     * let mut talon_left = TalonSRX::new(0);
     * let mut talon_rght = TalonSRX::new(1);
     * # let target_dist = 1023.0;
     * # let desired_robot_heading = 0.0;
     * talon_left.follow(&talon_rght, FollowerType::AuxOutput1);
     * talon_rght.set(ControlMode::MotionMagic, target_dist, Demand::AuxPID(desired_robot_heading));
     * ```
     */
    fn set(&mut self, mode: ControlMode, demand0: f64, demand1: Demand) {
        // NB: This does not store the control mode and setpoint to avoid several complications.
        let demand0 = match mode {
            // did caller specify device ID
            ControlMode::Follower if 0.0 <= demand0 && demand0 <= 62.0 => {
                f64::from(((self.base_id() as u32 >> 16) << 8) | (demand0 as u32))
            }
            _ => demand0,
        };
        _set(self.handle(), mode, demand0, demand1)
    }

    /// Neutral the motor output by setting control mode to disabled.
    fn neutral_output(&mut self) {
        self.set(ControlMode::Disabled, 0.0, Demand::Neutral)
    }
    /// Sets the mode of operation during neutral throttle output.
    fn set_neutral_mode(&mut self, neutral_mode: NeutralMode) {
        unsafe { c_MotController_SetNeutralMode(self.handle(), neutral_mode as _) }
    }

    /**
     * Sets the phase of the sensor. Use when controller forward/reverse output
     * doesn't correlate to appropriate forward/reverse reading of sensor.
     * Pick a value so that positive PercentOutput yields a positive change in sensor.
     * After setting this, user can freely call [`set_inverted`] with any value.
     *
     * # Parameters
     *
     * - `phase_sensor`: Indicates whether to invert the phase of the sensor.
     *
     * [`set_inverted`]: #method.set_inverted
     */
    fn set_sensor_phase(&mut self, phase_sensor: bool) {
        unsafe { c_MotController_SetSensorPhase(self.handle(), phase_sensor) }
    }
    /**
     * Inverts the hbridge output of the motor controller.
     *
     * This does not impact sensor phase and should not be used to correct sensor polarity.
     *
     * This will invert the hbridge output but NOT the LEDs.
     * This ensures....
     *  - Green LEDs always represents positive request from robot-controller/closed-looping mode.
     *  - Green LEDs correlates to forward limit switch.
     *  - Green LEDs correlates to forward soft limit.
     */
    fn set_inverted(&mut self, invert: InvertType) {
        unsafe { c_MotController_SetInverted_2(self.handle(), invert as _) }
    }

    fn config_factory_default(&mut self, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_ConfigFactoryDefault(self.handle(), timeout_ms) }
    }

    fn config_open_loop_ramp(
        &mut self,
        seconds_from_neutral_to_full: f64,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigOpenLoopRamp(
                self.handle(),
                seconds_from_neutral_to_full,
                timeout_ms,
            )
        }
    }
    fn config_closed_loop_ramp(
        &mut self,
        seconds_from_neutral_to_full: f64,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigClosedLoopRamp(
                self.handle(),
                seconds_from_neutral_to_full,
                timeout_ms,
            )
        }
    }

    fn config_peak_output_forward(&mut self, percent_out: f64, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_ConfigPeakOutputForward(self.handle(), percent_out, timeout_ms) }
    }
    fn config_peak_output_reverse(&mut self, percent_out: f64, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_ConfigPeakOutputReverse(self.handle(), percent_out, timeout_ms) }
    }

    fn config_nominal_output_forward(&mut self, percent_out: f64, timeout_ms: i32) -> ErrorCode {
        unsafe {
            c_MotController_ConfigNominalOutputForward(self.handle(), percent_out, timeout_ms)
        }
    }
    fn config_nominal_output_reverse(&mut self, percent_out: f64, timeout_ms: i32) -> ErrorCode {
        unsafe {
            c_MotController_ConfigNominalOutputReverse(self.handle(), percent_out, timeout_ms)
        }
    }

    fn config_neutral_deadband(&mut self, percent_deadband: f64, timeout_ms: i32) -> ErrorCode {
        unsafe {
            c_MotController_ConfigNeutralDeadband(self.handle(), percent_deadband, timeout_ms)
        }
    }

    /**
     * Configures the Voltage Compensation saturation voltage.
     *
     * # Parameters
     *
     * - `voltage`: The max voltage to apply to the hbridge when voltage
     *   compensation is enabled.  For example, if 10 (volts) is specified
     *   and a TalonSRX is commanded to 0.5 (PercentOutput, closed-loop, etc)
     *   then the TalonSRX will attempt to apply a duty-cycle to produce 5V.
     * - `timeout_ms`: Timeout value in ms.
     *   If nonzero, function will wait for config success and report an error if it times out.
     *   If zero, no blocking or checking is performed.
     */
    fn config_voltage_comp_saturation(&mut self, voltage: f64, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_ConfigVoltageCompSaturation(self.handle(), voltage, timeout_ms) }
    }
    /// Configures the voltage measurement filter.
    ///
    /// # Parameters
    /// - `filter_window_samples`: Number of samples in the rolling average of voltage measurement.
    fn config_voltage_measurement_filter(
        &mut self,
        filter_window_samples: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigVoltageMeasurementFilter(
                self.handle(),
                filter_window_samples,
                timeout_ms,
            )
        }
    }
    /// Enable voltage compensation.
    /// If enabled, voltage compensation works in all control modes.
    fn enable_voltage_compensation(&mut self, enable: bool) {
        unsafe { c_MotController_EnableVoltageCompensation(self.handle(), enable) }
    }

    fn inverted(&self) -> Result<bool> {
        // TODO: cache invert type
        cci_get!(c_MotController_GetInverted(self.handle(), _: bool))
    }

    fn bus_voltage(&self) -> Result<f64> {
        cci_get!(c_MotController_GetBusVoltage(self.handle(), _: f64))
    }
    /// Gets the output percentage of the motor controller, in the interval [-1,+1].
    fn motor_output_percent(&self) -> Result<f64> {
        cci_get!(c_MotController_GetMotorOutputPercent(self.handle(), _: f64))
    }
    fn motor_output_voltage(&self) -> Result<f64> {
        Ok(self.bus_voltage()? * self.motor_output_percent()?)
    }

    // output current moved to TalonSRX

    /// Gets the temperature of the motor controller in degrees Celsius.
    fn temperature(&self) -> Result<f64> {
        cci_get!(c_MotController_GetTemperature(self.handle(), _: f64))
    }

    /**
     * Select the remote feedback device for the motor controller.
     * Most CTRE CAN motor controllers will support remote sensors over CAN.
     */
    fn config_selected_feedback_sensor(
        &mut self,
        feedback_device: RemoteFeedbackDevice,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigSelectedFeedbackSensor(
                self.handle(),
                feedback_device as _,
                pid_loop as _,
                timeout_ms,
            )
        }
    }
    /**
     * The Feedback Coefficient is a scalar applied to the value of the
     * feedback sensor.  Useful when you need to scale your sensor values
     * within the closed-loop calculations.  Default value is 1.
     *
     * Selected Feedback Sensor register in firmware is the decoded sensor value
     * multiplied by the Feedback Coefficient.
     *
     * # Parameters
     *
     * - `coefficient`: Feedback Coefficient value.  Maximum value of 1.
     *   Resolution is 1/(2^16).  Cannot be 0.
     * - `pid_loop`: Primary or auxiliary closed-loop.
     * - `timeout_ms`: Timeout value in ms.
     *   If nonzero, function will wait for config success and report an error if it times out.
     *   If zero, no blocking or checking is performed.
     */
    fn config_selected_feedback_coefficient(
        &mut self,
        coefficient: f64,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigSelectedFeedbackCoefficient(
                self.handle(),
                coefficient,
                pid_loop as _,
                timeout_ms,
            )
        }
    }

    /**
     * Select what remote device and signal to assign to Remote Sensor 0 or Remote Sensor 1.
     * After binding a remote device and signal to Remote Sensor X, you may select Remote Sensor X
     * as a PID source for closed-loop features.
     */
    fn config_remote_feedback_filter(
        &mut self,
        device_id: i32,
        remote_sensor_source: RemoteSensorSource,
        remote_ordinal: RemoteFilter,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigRemoteFeedbackFilter(
                self.handle(),
                device_id,
                remote_sensor_source as _,
                remote_ordinal as _,
                timeout_ms,
            )
        }
    }

    /**
     * Select what sensor term should be bound to switch feedback device.
     *
     * - Sensor Sum = Sensor Sum Term 0 - Sensor Sum Term 1
     * - Sensor Difference = Sensor Diff Term 0 - Sensor Diff Term 1
     *
     * The four terms are specified with this routine.  Then Sensor
     * Sum/Difference can be selected for closed-looping.
     */
    fn config_sensor_term(
        &mut self,
        sensor_term: SensorTerm,
        feedback_device: RemoteFeedbackDevice,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigSensorTerm(
                self.handle(),
                sensor_term as _,
                feedback_device as _,
                timeout_ms,
            )
        }
    }

    /// Get the selected sensor position (in raw sensor units).
    fn get_selected_sensor_position(&self, idx: PIDLoop) -> Result<i32> {
        cci_get!(c_MotController_GetSelectedSensorPosition(self.handle(), _: i32, idx as _))
    }
    fn get_selected_sensor_velocity(&self, idx: PIDLoop) -> Result<i32> {
        cci_get!(c_MotController_GetSelectedSensorVelocity(self.handle(), _: i32, idx as _))
    }
    fn set_selected_sensor_position(
        &mut self,
        sensor_pos: i32,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_SetSelectedSensorPosition(
                self.handle(),
                sensor_pos,
                pid_loop as _,
                timeout_ms,
            )
        }
    }

    fn set_control_frame_period(&mut self, frame: ControlFrame, period_ms: i32) -> ErrorCode {
        unsafe { c_MotController_SetControlFramePeriod(self.handle(), frame as _, period_ms) }
    }
    fn set_status_frame_period(
        &mut self,
        frame: StatusFrame,
        period_ms: u8,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_SetStatusFramePeriod(self.handle(), frame as _, period_ms, timeout_ms)
        }
    }
    fn get_status_frame_period(&self, frame: StatusFrame, timeout_ms: i32) -> Result<i32> {
        cci_get! {
            c_MotController_GetStatusFramePeriod(self.handle(), frame as _, _: i32, timeout_ms)
        }
    }

    /**
     * Configures the period of each velocity sample.
     * Every 1ms a position value is sampled, and the delta between that sample
     * and the position sampled kPeriod ms ago is inserted into a filter.
     * kPeriod is configured with this function.
     */
    fn config_velocity_measurement_period(
        &mut self,
        period: VelocityMeasPeriod,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigVelocityMeasurementPeriod(self.handle(), period as _, timeout_ms)
        }
    }
    /// Sets the number of velocity samples used in the rolling average velocity measurement.
    ///
    /// Valid window sizes are 1, 2, 4, 8, 16 and 32.
    /// If an invalid value is specified, it will truncate to the nearest supported.
    fn config_velocity_measurement_window(
        &mut self,
        window_size: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigVelocityMeasurementWindow(self.handle(), window_size, timeout_ms)
        }
    }

    /**
     * Configures the forward limit switch for a remote source.
     * For example, a CAN motor controller may need to monitor the Limit-F pin
     * of another Talon or CANifier.
     *
     * # Parameters
     *
     * - `type_`: Remote limit switch source.
     *   User can choose between a remote Talon SRX, CANifier, or deactivate the feature.
     * - `normal_open_or_close`: Setting for normally open, normally closed, or disabled.
     *   This setting matches the web-based configuration drop down.
     * - `device_id`: Device ID of remote source (Talon SRX or CANifier device ID).
     * - `timeout_ms`: Timeout value in ms.
     *   If nonzero, function will wait for config success and report an error if it times out.
     *   If zero, no blocking or checking is performed.
     */
    fn config_forward_limit_switch_source(
        &mut self,
        type_: RemoteLimitSwitchSource,
        normal_open_or_close: LimitSwitchNormal,
        device_id: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigForwardLimitSwitchSource(
                self.handle(),
                type_ as _,
                normal_open_or_close as _,
                device_id,
                timeout_ms,
            )
        }
    }
    /**
     * Configures the reverse limit switch for a remote source.
     * For example, a CAN motor controller may need to monitor the Limit-R pin
     * of another Talon or CANifier.
     *
     * # Parameters
     *
     * - `type_`: Remote limit switch source.
     *   User can choose between a remote Talon SRX, CANifier, or deactivate the feature.
     * - `normal_open_or_close`: Setting for normally open, normally closed, or disabled.
     *   This setting matches the web-based configuration drop down.
     * - `device_id`: Device ID of remote source (Talon SRX or CANifier device ID).
     * - `timeout_ms`: Timeout value in ms.
     *   If nonzero, function will wait for config success and report an error if it times out.
     *   If zero, no blocking or checking is performed.
     */
    fn config_reverse_limit_switch_source(
        &mut self,
        type_: RemoteLimitSwitchSource,
        normal_open_or_close: LimitSwitchNormal,
        device_id: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigReverseLimitSwitchSource(
                self.handle(),
                type_ as _,
                normal_open_or_close as _,
                device_id,
                timeout_ms,
            )
        }
    }
    /**
     * Sets the enable state for limit switches.
     *
     * This routine can be used to *disable* the limit switch feature.
     * This is helpful to force off the limit switch detection.
     * For example, a module can leave limit switches enable for home-ing
     * a continuous mechanism, and once done this routine can force off
     * disabling of the motor controller.
     *
     * Limit switches must be enabled using the config routines first.
     */
    fn override_limit_switches_enable(&mut self, enable: bool) {
        unsafe { c_MotController_OverrideLimitSwitchesEnable(self.handle(), enable) }
    }

    fn config_forward_soft_limit_threshold(
        &mut self,
        forward_sensor_limit: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigForwardSoftLimitThreshold(
                self.handle(),
                forward_sensor_limit,
                timeout_ms,
            )
        }
    }
    fn config_reverse_soft_limit_threshold(
        &mut self,
        reverse_sensor_limit: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigReverseSoftLimitThreshold(
                self.handle(),
                reverse_sensor_limit,
                timeout_ms,
            )
        }
    }
    fn config_forward_soft_limit_enable(&mut self, enable: bool, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_ConfigForwardSoftLimitEnable(self.handle(), enable, timeout_ms) }
    }
    fn config_reverse_soft_limit_enable(&mut self, enable: bool, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_ConfigReverseSoftLimitEnable(self.handle(), enable, timeout_ms) }
    }
    fn override_soft_limits_enable(&mut self, enable: bool) {
        unsafe { c_MotController_OverrideSoftLimitsEnable(self.handle(), enable) }
    }

    // current limiting is Talon-specific

    fn config_kp(&mut self, slot_idx: PIDSlot, value: f64, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_Config_kP(self.handle(), slot_idx as _, value, timeout_ms) }
    }
    fn config_ki(&mut self, slot_idx: PIDSlot, value: f64, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_Config_kI(self.handle(), slot_idx as _, value, timeout_ms) }
    }
    fn config_kd(&mut self, slot_idx: PIDSlot, value: f64, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_Config_kD(self.handle(), slot_idx as _, value, timeout_ms) }
    }
    fn config_kf(&mut self, slot_idx: PIDSlot, value: f64, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_Config_kF(self.handle(), slot_idx as _, value, timeout_ms) }
    }
    fn config_integral_zone(&mut self, slot: PIDSlot, izone: i32, timeout_ms: i32) -> ErrorCode {
        unsafe {
            c_MotController_Config_IntegralZone(
                self.handle(),
                slot as _,
                f64::from(izone), // idek both C++ and Java do this too
                timeout_ms,
            )
        }
    }
    fn config_allowable_closed_loop_error(
        &mut self,
        slot_idx: PIDSlot,
        allowable_closed_loop_error: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigAllowableClosedloopError(
                self.handle(),
                slot_idx as _,
                allowable_closed_loop_error,
                timeout_ms,
            )
        }
    }
    fn config_max_integral_accumulator(
        &mut self,
        slot_idx: PIDSlot,
        iaccum: f64,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigMaxIntegralAccumulator(
                self.handle(),
                slot_idx as _,
                iaccum,
                timeout_ms,
            )
        }
    }
    fn config_closed_loop_peak_output(
        &mut self,
        slot_idx: PIDSlot,
        percent_out: f64,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigClosedLoopPeakOutput(
                self.handle(),
                slot_idx as _,
                percent_out,
                timeout_ms,
            )
        }
    }
    fn config_closed_loop_period(
        &mut self,
        slot_idx: PIDSlot,
        loop_time_ms: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigClosedLoopPeriod(
                self.handle(),
                slot_idx as _,
                loop_time_ms,
                timeout_ms,
            )
        }
    }
    /**
     * Configures the Polarity of the Auxiliary PID (PID1).
     *
     * Standard Polarity:
     *  - Primary Output = PID0 + PID1
     *  - Auxiliary Output = PID0 - PID1
     *
     * Inverted Polarity:
     *  - Primary Output = PID0 - PID1
     *  - Auxiliary Output = PID0 + PID1
     */
    fn config_aux_pid_polarity(&mut self, polarity: AuxPIDPolarity, timeout_ms: i32) -> ErrorCode {
        self.config_set_parameter(
            ParamEnum::PIDLoopPolarity,
            polarity as isize as f64,
            0,
            1,
            timeout_ms,
        )
    }
    fn set_integral_accumulator(
        &mut self,
        iaccum: f64,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_SetIntegralAccumulator(self.handle(), iaccum, pid_loop as _, timeout_ms)
        }
    }
    fn get_closed_loop_error(&self, pid_loop: PIDLoop) -> Result<i32> {
        cci_get!(c_MotController_GetClosedLoopError(self.handle(), _: i32, pid_loop as _))
    }
    fn get_integral_accumulator(&self, pid_loop: PIDLoop) -> Result<f64> {
        cci_get!(c_MotController_GetIntegralAccumulator(self.handle(), _: f64, pid_loop as _))
    }
    /// Gets the derivative of the closed-loop error.
    fn get_error_derivative(&self, pid_loop: PIDLoop) -> Result<f64> {
        cci_get!(c_MotController_GetErrorDerivative(self.handle(), _: f64, pid_loop as _))
    }
    /// Selects which profile slot to use for closed-loop control.
    fn select_profile_slot(&self, slot_idx: PIDSlot, pid_loop: PIDLoop) -> ErrorCode {
        unsafe { c_MotController_SelectProfileSlot(self.handle(), slot_idx as _, pid_loop as _) }
    }
    fn get_closed_loop_target(&self, pid_loop: PIDLoop) -> Result<i32> {
        cci_get!(c_MotController_GetClosedLoopTarget(self.handle(), _: i32, pid_loop as _))
    }

    /// Gets the active trajectory target position using MotionMagic/MotionProfile control modes.
    fn active_trajectory_position(&self) -> Result<i32> {
        cci_get!(c_MotController_GetActiveTrajectoryPosition(self.handle(), _: i32))
    }
    fn get_active_trajectory_position(&self, pid_idx: PIDLoop) -> Result<i32> {
        cci_get!(c_MotController_GetActiveTrajectoryPosition_3(self.handle(), _: i32, pid_idx as _))
    }
    /// Gets the active trajectory target velocity using MotionMagic/MotionProfile control modes.
    fn active_trajectory_velocity(&self) -> Result<i32> {
        cci_get!(c_MotController_GetActiveTrajectoryVelocity(self.handle(), _: i32))
    }
    fn get_active_trajectory_velocity(&self, pid_idx: PIDLoop) -> Result<i32> {
        cci_get!(c_MotController_GetActiveTrajectoryVelocity_3(self.handle(), _: i32, pid_idx as _))
    }
    /// Gets the active trajectory target heading using MotionMagic/MotionProfile control modes.
    #[deprecated(
        since = "0.8.0",
        note = "use `get_active_trajectory_position(PIDLoop::Auxiliary)` instead"
    )]
    fn active_trajectory_heading(&self) -> Result<f64> {
        cci_get!(c_MotController_GetActiveTrajectoryHeading(self.handle(), _: f64))
    }
    fn get_active_trajectory_arb_feed_fwd(&self, pid_idx: PIDLoop) -> Result<f64> {
        cci_get! {
            c_MotController_GetActiveTrajectoryArbFeedFwd_3(self.handle(), _: f64, pid_idx as _)
        }
    }

    /// Sets the Motion Magic Cruise Velocity.
    /// This is the peak target velocity that the motion magic curve generator can use.
    fn config_motion_cruise_velocity(
        &mut self,
        sensor_units_per_100ms: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigMotionCruiseVelocity(
                self.handle(),
                sensor_units_per_100ms,
                timeout_ms,
            )
        }
    }
    /// Sets the Motion Magic Acceleration.
    /// This is the target acceleration that the motion magic curve generator can use.
    fn config_motion_acceleration(
        &mut self,
        sensor_units_per_100ms_per_sec: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigMotionAcceleration(
                self.handle(),
                sensor_units_per_100ms_per_sec,
                timeout_ms,
            )
        }
    }
    /**
     * Sets the Motion Magic S Curve Strength.
     * Call this before using Motion Magic.
     * Modifying this during a Motion Magic action should be avoided.
     *
     * # Parameters
     *
     * - `curve_strength`: 0 to use Trapezoidal Motion Profile. [1,8] for S-Curve (greater value yields greater smoothing).
     */
    fn config_motion_s_curve_strength(
        &mut self,
        curve_strength: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigMotionSCurveStrength(self.handle(), curve_strength, timeout_ms)
        }
    }

    /// Clear the buffered motion profile in both motor controller's RAM (bottom),
    /// and in the API (top).
    fn clear_motion_profile_trajectories(&mut self) -> ErrorCode {
        unsafe { c_MotController_ClearMotionProfileTrajectories(self.handle()) }
    }
    /**
     * Retrieve just the buffer count for the api-level (top) buffer.
     * This routine performs no CAN or data structure lookups, so its fast and ideal
     * if caller needs to quickly poll the progress of trajectory points being
     * emptied into motor controller's RAM. Otherwise just use [`motion_profile_status`].
     *
     * [`motion_profile_status`]: #method.motion_profile_status
     */
    fn motion_profile_top_level_buffer_count(&self) -> Result<i32> {
        cci_get!(c_MotController_GetMotionProfileTopLevelBufferCount(self.handle(), _: i32))
    }
    /// Push another trajectory point into the top level buffer (which is emptied
    /// into the motor controller's bottom buffer as room allows).
    fn push_motion_profile_trajectory(&self, traj_pt: &TrajectoryPoint) -> ErrorCode {
        unsafe {
            c_MotController_PushMotionProfileTrajectory_3(
                self.handle(),
                traj_pt.position,
                traj_pt.velocity,
                traj_pt.arb_feed_fwd,
                traj_pt.auxiliary_pos,
                traj_pt.auxiliary_vel,
                traj_pt.auxiliary_arb_feed_fwd,
                traj_pt.profile_slot_select_0 as u32,
                traj_pt.profile_slot_select_1 as u32,
                traj_pt.is_last_point,
                traj_pt.zero_pos,
                traj_pt.time_dur,
                traj_pt.use_aux_pid,
            )
        }
    }

    /**
     * Simple one-shot firing of a complete MP.
     *
     * Starting in 2019, MPs can be fired by building a Buffered Trajectory Point Stream, and calling this routine.
     *
     * Once called, the motor controller software will automatically ...
     *
     * 1. Clear the firmware buffer of trajectory points.
     * 2. Clear the underrun flags
     * 3. Reset an index within the Buffered Trajectory Point Stream
     *    (so that the same profile can be run again and again).
     * 4. Start a background thread to manage MP streaming (if not already running).
     * 5. If current control mode...
     *    a. already matches `motion_prof_control_mode`, set MPE Output to "Hold".
     *    b. does not matches `motion_prof_control_mode`, apply `motion_prof_control_mode`
     *       and set MPE Output to "Disable".
     * 6. Stream the trajectory points into the device's firmware buffer.
     * 7. Once motor controller has at least `min_buffered_pts` worth in the firmware buffer,
     *    MP will automatically start (MPE Output set to "Enable").
     * 8. Wait until MP finishes, then transitions the Motion Profile Executor's output to "Hold".
     * 9. [`is_motion_profile_finished`] will now return true.
     *
     * Calling application can cancel MP by calling [`set`].
     * Otherwise do not call `set` until MP has completed.
     *
     * The legacy API from previous years requires the calling application to
     * pass points via [`process_motion_profile_buffer`] and [`push_motion_profile_trajectory`].
     * This is no longer required if using this API.
     *
     * # Parameters
     *
     * - `stream`: A buffer that will be used to stream the trajectory points.
     *   Caller can fill this container with the entire trajectory point, regardless of size.
     * - `min_buffered_pts`: Minimum number of firmware buffered points before starting MP.  
     *   Do not exceed device's firmware buffer capacity or MP will never fire
     *   (120 for Motion Profile, or 60 for Motion Profile Arc).
     *   Recommendation value for this would be five to ten samples depending
     *   on `time_dur` of the trajectory point.
     * - `motion_prof_control_mode`: Pass `MotionProfile` or `MotionProfileArc`.
     *
     * [`is_motion_profile_finished`]: #method.is_motion_profile_finished
     * [`set`]: #method.set
     * [`process_motion_profile_buffer`]: #method.process_motion_profile_buffer
     * [`push_motion_profile_trajectory`]: #method.push_motion_profile_trajectory
     */
    fn start_motion_profile(
        &mut self,
        stream: &BuffTrajPointStream,
        min_buffered_pts: u32,
        motion_prof_control_mode: ControlMode,
    ) -> ErrorCode {
        unsafe {
            c_MotController_StartMotionProfile(
                self.handle(),
                stream.handle,
                min_buffered_pts,
                motion_prof_control_mode,
            )
        }
    }
    /// Determine if the running MP (from [`start_motion_profile`]) is complete.
    ///
    /// [`start_motion_profile`]: #method.start_motion_profile
    fn is_motion_profile_finished(&self) -> Result<bool> {
        cci_get!(c_MotController_IsMotionProfileFinished(self.handle(), _: bool))
    }

    /**
     * Retrieve just the buffer full for the api-level (top) buffer.
     * This routine performs no CAN or data structure lookups, so its fast and ideal
     * if caller needs to quickly poll. Otherwise just use [`motion_profile_status`].
     *
     * [`motion_profile_status`]: #method.motion_profile_status
     */
    fn is_motion_profile_top_level_buffer_full(&self) -> Result<bool> {
        cci_get!(c_MotController_IsMotionProfileTopLevelBufferFull(self.handle(), _: bool))
    }
    /**
     * This must be called periodically to funnel the trajectory points from the
     * API's top level buffer to the controller's bottom level buffer.  Recommendation
     * is to call this twice as fast as the execution rate of the motion profile.
     * So if MP is running with 20ms trajectory points, try calling this routine
     * every 10ms.  All motion profile functions are thread-safe through the use of
     * a mutex, so there is no harm in having the caller utilize threading.
     */
    fn process_motion_profile_buffer(&self) {
        unsafe { c_MotController_ProcessMotionProfileBuffer(self.handle()) };
    }
    /**
     * Retrieve all status information.
     * For best performance, Caller can snapshot all status information
     * regarding the motion profile executer.
     */
    fn motion_profile_status_into(&self, status_to_fill: &mut MotionProfileStatus) -> ErrorCode {
        unsafe { _motion_profile_status(self.handle(), status_to_fill) }
    }
    /// Get all motion profile status information.  This returns a new MotionProfileStatus.
    fn motion_profile_status(&self) -> Result<MotionProfileStatus> {
        let mut status_to_fill = mem::MaybeUninit::uninit();
        let code = unsafe { _motion_profile_status(self.handle(), status_to_fill.as_mut_ptr()) };
        match code {
            ErrorCode::OK => Ok(unsafe { status_to_fill.assume_init() }),
            _ => Err(code),
        }
    }
    /// Clear the "Has Underrun" flag.
    /// Typically this is called after application has confirmed an underrun had occured.
    fn clear_motion_profile_has_underrun(&mut self, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_ClearMotionProfileHasUnderrun(self.handle(), timeout_ms) }
    }
    /**
     * Calling application can opt to speed up the handshaking between the robot API
     * and the controller to increase the download rate of the controller's Motion Profile.
     * Ideally the period should be no more than half the period of a trajectory
     * point.
     */
    fn change_motion_control_frame_period(&mut self, period_ms: i32) -> ErrorCode {
        unsafe { c_MotController_ChangeMotionControlFramePeriod(self.handle(), period_ms) }
    }
    /**
     * When trajectory points are processed in the motion profile executer, the MPE determines
     * how long to apply the active trajectory point by summing `base_traj_duration_ms` with the
     * `time_dur` of the trajectory point (see [`TrajectoryPoint`]).
     *
     * This allows general selection of the execution rate of the points with 1ms resolution,
     * while allowing some degree of change from point to point.
     *
     * # Parameters
     *
     * - `base_traj_duration_ms`: The base duration time of every trajectory point.
     *   This is summed with the trajectory points unique `time_dur`.
     * - `timeout_ms`: Timeout value in ms.
     *   If nonzero, function will wait for config success and report an error if it times out.
     *   If zero, no blocking or checking is performed.
     *
     * [`TrajectoryPoint`]: ../motion/struct.TrajectoryPoint.html
     */
    fn config_motion_profile_trajectory_period(
        &mut self,
        base_traj_duration_ms: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigMotionProfileTrajectoryPeriod(
                self.handle(),
                base_traj_duration_ms,
                timeout_ms,
            )
        }
    }
    fn config_motion_profile_trajectory_interpolation_enable(
        &mut self,
        enable: bool,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigMotionProfileTrajectoryInterpolationEnable(
                self.handle(),
                enable,
                timeout_ms,
            )
        }
    }

    // Feedback Device Interaction Settings
    // XXX: do these really belong here? these seem an enhanced motor controller only thing
    fn config_feedback_not_continuous(
        &mut self,
        feedback_not_continuous: bool,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigFeedbackNotContinuous(
                self.handle(),
                feedback_not_continuous,
                timeout_ms,
            )
        }
    }
    fn config_remote_sensor_closed_loop_disable_neutral_on_los(
        &mut self,
        remote_sensor_closed_loop_disable_neutral_on_los: bool,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigRemoteSensorClosedLoopDisableNeutralOnLOS(
                self.handle(),
                remote_sensor_closed_loop_disable_neutral_on_los,
                timeout_ms,
            )
        }
    }
    fn config_clear_position_on_limit_f(
        &mut self,
        clear_position_on_limit_f: bool,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigClearPositionOnLimitF(
                self.handle(),
                clear_position_on_limit_f,
                timeout_ms,
            )
        }
    }
    fn config_clear_position_on_limit_r(
        &mut self,
        clear_position_on_limit_r: bool,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigClearPositionOnLimitR(
                self.handle(),
                clear_position_on_limit_r,
                timeout_ms,
            )
        }
    }
    fn config_clear_position_on_quad_idx(
        &mut self,
        clear_position_on_quad_idx: bool,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigClearPositionOnQuadIdx(
                self.handle(),
                clear_position_on_quad_idx,
                timeout_ms,
            )
        }
    }
    fn config_limit_switch_disable_neutral_on_los(
        &mut self,
        limit_switch_disable_neutral_on_los: bool,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigLimitSwitchDisableNeutralOnLOS(
                self.handle(),
                limit_switch_disable_neutral_on_los,
                timeout_ms,
            )
        }
    }
    fn config_soft_limit_disable_neutral_on_los(
        &mut self,
        soft_limit_disable_neutral_on_los: bool,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigSoftLimitDisableNeutralOnLOS(
                self.handle(),
                soft_limit_disable_neutral_on_los,
                timeout_ms,
            )
        }
    }
    fn config_pulse_width_period_edges_per_rot(
        &mut self,
        pulse_width_period_edges_per_rot: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigPulseWidthPeriod_EdgesPerRot(
                self.handle(),
                pulse_width_period_edges_per_rot,
                timeout_ms,
            )
        }
    }
    fn config_pulse_width_period_filter_window_sz(
        &mut self,
        pulse_width_period_filter_window_sz: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigPulseWidthPeriod_FilterWindowSz(
                self.handle(),
                pulse_width_period_filter_window_sz,
                timeout_ms,
            )
        }
    }

    /**
     * Gets the last error generated by this object.
     * Not all functions return an error code but can potentially report errors.
     * This function can be used to retrieve those error codes.
     */
    fn last_error(&self) -> ErrorCode {
        unsafe { c_MotController_GetLastError(self.handle()) }
    }

    fn faults(&self) -> Result<Faults> {
        Ok(Faults {
            bits: cci_get!(c_MotController_GetFaults(self.handle(), _: i32))?,
        })
    }
    fn sticky_faults(&self) -> Result<StickyFaults> {
        Ok(StickyFaults {
            bits: cci_get!(c_MotController_GetStickyFaults(self.handle(), _: i32))?,
        })
    }
    fn clear_sticky_faults(&mut self, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_ClearStickyFaults(self.handle(), timeout_ms) }
    }

    /// Gets the firmware version of the device as a single hexadecimal integer.
    /// For example, version 1-dot-2 is encoded as 0x0102.
    fn firmware_version(&self) -> Result<i32> {
        cci_get!(c_MotController_GetFirmwareVersion(self.handle(), _: i32))
    }
    /// Returns true if the device has reset since last call.
    fn has_reset_occurred(&self) -> Result<bool> {
        cci_get!(c_MotController_HasResetOccurred(self.handle(), _: bool))
    }

    /**
     * Sets the value of a custom parameter. This is for arbitrary use.
     *
     * Sometimes it is necessary to save calibration/limit/target
     * information in the device. Particularly if the
     * device is part of a subsystem that can be replaced.
     */
    fn config_set_custom_param(
        &mut self,
        value: i32,
        param_index: CustomParam,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigSetCustomParam(self.handle(), value, param_index as _, timeout_ms)
        }
    }
    /// Gets the value of a custom parameter.
    fn config_get_custom_param(&self, param_index: CustomParam, timout_ms: i32) -> Result<i32> {
        cci_get!(
            c_MotController_ConfigGetCustomParam(self.handle(), _: i32, param_index as _, timout_ms)
        )
    }

    /**
     * Sets a parameter. Generally this is not used.
     * This can be utilized in
     * - Using new features without updating API installation.
     * - Errata workarounds to circumvent API implementation.
     * - Allows for rapid testing / unit testing of firmware.
     */
    fn config_set_parameter(
        &mut self,
        param: ParamEnum,
        value: f64,
        sub_value: u8,
        ordinal: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigSetParameter(
                self.handle(),
                param.0 as _,
                value,
                sub_value as _,
                ordinal,
                timeout_ms,
            )
        }
    }
    fn config_get_parameter(&self, param: ParamEnum, ordinal: i32, timeout_ms: i32) -> Result<f64> {
        cci_get!(c_MotController_ConfigGetParameter(
            self.handle(),
            param.0 as _,
            _: f64,
            ordinal,
            timeout_ms,
        ))
    }

    /**
     * Set the control mode and output value so that this motor controller will
     * follow another motor controller.
     * Currently supports following Victor SPX and Talon SRX.
     *
     * # Parameters
     *
     * - `master_to_follow`: Motor Controller object to follow.
     * - `follower_type`: Type of following control.
     *   Use AuxOutput1 to follow the master device's auxiliary output 1.
     *   Use PercentOutput for standard follower mode.
     */
    fn follow(&mut self, master_to_follow: &impl MotorController, follower_type: FollowerType)
    where
        Self: Sized,
    {
        let base_id = master_to_follow.base_id();
        let id24: i32 = ((base_id >> 0x10) << 8) | (base_id & 0xFF);

        match follower_type {
            FollowerType::PercentOutput => {
                self.set(ControlMode::Follower, f64::from(id24), Demand::Neutral)
            }
            FollowerType::AuxOutput1 => {
                // follow the motor controller, but set the aux flag
                // to ensure we follow the processed output
                self.set(ControlMode::Follower, f64::from(id24), Demand::AuxPID(0.0))
            }
        }
    }

    /// Configures all slot persistent settings.
    fn configure_slot(
        &mut self,
        slot: &SlotConfiguration,
        slot_idx: PIDSlot,
        timeout_ms: i32,
    ) -> ErrorCode {
        self.config_kp(slot_idx, slot.kp, timeout_ms)
            .or(self.config_ki(slot_idx, slot.ki, timeout_ms))
            .or(self.config_kd(slot_idx, slot.kd, timeout_ms))
            .or(self.config_kf(slot_idx, slot.kf, timeout_ms))
            .or(self.config_integral_zone(slot_idx, slot.integral_zone, timeout_ms))
            .or(self.config_allowable_closed_loop_error(
                slot_idx,
                slot.allowable_closed_loop_error,
                timeout_ms,
            ))
            .or(self.config_max_integral_accumulator(
                slot_idx,
                slot.max_integral_accumulator,
                timeout_ms,
            ))
            .or(self.config_closed_loop_peak_output(
                slot_idx,
                slot.closed_loop_peak_output,
                timeout_ms,
            ))
            .or(self.config_closed_loop_period(slot_idx, slot.closed_loop_period, timeout_ms))
    }
    /// Gets all slot persistent settings.
    fn get_slot_configs(&self, slot_idx: PIDSlot, timeout_ms: i32) -> Result<SlotConfiguration> {
        let slot_idx = slot_idx as i32;
        Ok(SlotConfiguration {
            kp: self.config_get_parameter(ParamEnum::ProfileParamSlot_P, slot_idx, timeout_ms)?,
            ki: self.config_get_parameter(ParamEnum::ProfileParamSlot_I, slot_idx, timeout_ms)?,
            kd: self.config_get_parameter(ParamEnum::ProfileParamSlot_D, slot_idx, timeout_ms)?,
            kf: self.config_get_parameter(ParamEnum::ProfileParamSlot_F, slot_idx, timeout_ms)?,
            integral_zone: self.config_get_parameter(
                ParamEnum::ProfileParamSlot_IZone,
                slot_idx,
                timeout_ms,
            )? as i32,
            allowable_closed_loop_error: self.config_get_parameter(
                ParamEnum::ProfileParamSlot_AllowableErr,
                slot_idx,
                timeout_ms,
            )? as i32,
            max_integral_accumulator: self.config_get_parameter(
                ParamEnum::ProfileParamSlot_MaxIAccum,
                slot_idx,
                timeout_ms,
            )?,
            closed_loop_peak_output: self.config_get_parameter(
                ParamEnum::ProfileParamSlot_PeakOutput,
                slot_idx,
                timeout_ms,
            )?,
            closed_loop_period: self.config_get_parameter(
                ParamEnum::PIDLoopPeriod,
                slot_idx,
                timeout_ms,
            )? as i32,
        })
    }

    /// Configures all filter persistent settings.
    fn configure_filter(
        &mut self,
        filter: &FilterConfiguration,
        ordinal: RemoteFilter,
        timeout_ms: i32,
    ) -> ErrorCode {
        self.config_remote_feedback_filter(
            filter.remote_sensor_device_id,
            filter.remote_sensor_source,
            ordinal,
            timeout_ms,
        )
    }
    /// Gets all filter persistent settings.
    fn get_filter_configs(
        &self,
        ordinal: RemoteFilter,
        timeout_ms: i32,
    ) -> Result<FilterConfiguration> {
        Ok(FilterConfiguration {
            remote_sensor_device_id: self.config_get_parameter(
                ParamEnum::RemoteSensorDeviceID,
                ordinal as _,
                timeout_ms,
            )? as i32,
            remote_sensor_source: f64_to_enum!(self.config_get_parameter(
                ParamEnum::RemoteSensorSource,
                ordinal as _,
                timeout_ms,
            )? => RemoteSensorSource),
        })
    }

    #[doc(hidden)]
    /// Configures all base PID set persistent settings.
    fn base_configure_pid(
        &mut self,
        pid: &BasePIDSetConfiguration,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> ErrorCode {
        self.config_selected_feedback_coefficient(
            pid.selected_feedback_coefficient,
            pid_loop as _,
            timeout_ms,
        )
    }
    #[doc(hidden)]
    /// Gets all base PID set persistent settings.
    fn base_get_pid_configs(
        &self,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> Result<BasePIDSetConfiguration> {
        Ok(BasePIDSetConfiguration {
            selected_feedback_coefficient: self.config_get_parameter(
                ParamEnum::SelectedSensorCoefficient,
                pid_loop as _,
                timeout_ms,
            )?,
        })
    }

    #[doc(hidden)]
    /// Configures all base persistent settings.
    fn base_config_all_settings(
        &mut self,
        all_configs: &BaseMotorControllerConfiguration,
        timeout_ms: i32,
    ) -> ErrorCode {
        self.config_factory_default(timeout_ms)
            // general output shaping
            .or(self.config_open_loop_ramp(all_configs.open_loop_ramp, timeout_ms))
            .or(self.config_closed_loop_ramp(all_configs.closed_loop_ramp, timeout_ms))
            .or(self.config_peak_output_forward(all_configs.peak_output_forward, timeout_ms))
            .or(self.config_peak_output_reverse(all_configs.peak_output_reverse, timeout_ms))
            .or(self.config_nominal_output_forward(all_configs.nominal_output_forward, timeout_ms))
            .or(self.config_nominal_output_reverse(all_configs.nominal_output_reverse, timeout_ms))
            .or(self.config_neutral_deadband(all_configs.neutral_deadband, timeout_ms))
            // voltage compensation
            .or(self
                .config_voltage_comp_saturation(all_configs.voltage_comp_saturation, timeout_ms))
            .or(self.config_voltage_measurement_filter(
                all_configs.voltage_measurement_filter,
                timeout_ms,
            ))
            // velocity signal conditioning
            .or(self.config_velocity_measurement_period(
                all_configs.velocity_measurement_period,
                timeout_ms,
            ))
            .or(self.config_velocity_measurement_window(
                all_configs.velocity_measurement_window,
                timeout_ms,
            ))
            // soft limit
            .or(self.config_forward_soft_limit_threshold(
                all_configs.forward_soft_limit_threshold,
                timeout_ms,
            ))
            .or(self.config_reverse_soft_limit_threshold(
                all_configs.reverse_soft_limit_threshold,
                timeout_ms,
            ))
            .or(self.config_forward_soft_limit_enable(
                all_configs.forward_soft_limit_enable,
                timeout_ms,
            ))
            .or(self.config_reverse_soft_limit_enable(
                all_configs.reverse_soft_limit_enable,
                timeout_ms,
            ))
            // limit switch not in base
            // current lim not in base
            // slots
            .or(self.configure_slot(&all_configs.slot_0, PIDSlot::S0, timeout_ms))
            .or(self.configure_slot(&all_configs.slot_1, PIDSlot::S1, timeout_ms))
            .or(self.configure_slot(&all_configs.slot_2, PIDSlot::S2, timeout_ms))
            .or(self.configure_slot(&all_configs.slot_3, PIDSlot::S3, timeout_ms))
            // auxiliary closed loop polarity
            .or(self.config_aux_pid_polarity(all_configs.aux_pid_polarity, timeout_ms))
            // remote feedback filters
            .or(self.configure_filter(&all_configs.filter_0, RemoteFilter::S0, timeout_ms))
            .or(self.configure_filter(&all_configs.filter_1, RemoteFilter::S1, timeout_ms))
            // motion profile settings used in Motion Magic
            .or(self.config_motion_cruise_velocity(all_configs.motion_cruise_velocity, timeout_ms))
            .or(self.config_motion_acceleration(all_configs.motion_acceleration, timeout_ms))
            // motion profile buffer
            .or(self.config_motion_profile_trajectory_period(
                all_configs.motion_profile_trajectory_period,
                timeout_ms,
            ))
            // custom persistent params
            .or(self.config_set_custom_param(
                all_configs.custom_param.0,
                CustomParam::A,
                timeout_ms,
            ))
            .or(self.config_set_custom_param(
                all_configs.custom_param.1,
                CustomParam::B,
                timeout_ms,
            ))
        // ...
    }
    #[doc(hidden)]
    fn base_get_all_configs(&self, _timeout_ms: i32) -> Result<BaseMotorControllerConfiguration> {
        todo!();
    }
}

/// Extension of `MotorController` with configuration-related methods.
pub trait MotorControllerConfig: MotorController + ConfigAll {
    type PIDSetConfiguration;

    /// Configures all PID set persistent settings.
    fn configure_pid(
        &mut self,
        pid: &Self::PIDSetConfiguration,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> ErrorCode;

    /// Gets all PID set persistent settings.
    fn get_pid_configs(
        &self,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> Result<Self::PIDSetConfiguration>;
}

/// An interface to raw sensor values from a motor controller,
/// regardless of whether they are actually being used for feedback.
///
/// Note that by default these are only updated every 160ms.
/// Prefer the `MotorController` selected sensor methods if possible.
pub trait SensorCollection: MotorController {
    /**
     * Get the position of whatever is in the analog pin of the Talon,
     * regardless of whether it is actually being used for feedback.
     *
     * # Returns
     * Returns the 24bit analog value.
     *
     * The bottom ten bits is the ADC (0 - 1023) on the analog pin of the Talon.
     * The upper 14 bits tracks the overflows and underflows (continuous sensor).
     */
    fn analog_in(&self) -> Result<i32> {
        cci_get!(c_MotController_GetAnalogIn(self.handle(), _: i32))
    }
    fn set_analog_position(&mut self, new_position: i32, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_SetAnalogPosition(self.handle(), new_position, timeout_ms) }
    }
    /// Get the position of whatever is in the analog pin of the Talon,
    /// regardless of whether it is actually being used for feedback.
    ///
    /// Returns the ADC (0 - 1023) on analog pin of the Talon.
    fn analog_in_raw(&self) -> Result<i32> {
        cci_get!(c_MotController_GetAnalogInRaw(self.handle(), _: i32))
    }
    /// Get the velocity of whatever is in the analog pin of the Talon,
    /// regardless of whether it is actually being used for feedback.
    ///
    /// Returns the speed in units per 100ms where 1024 units is one rotation.
    fn analog_in_vel(&self) -> Result<i32> {
        cci_get!(c_MotController_GetAnalogInVel(self.handle(), _: i32))
    }
    fn quadrature_position(&self) -> Result<i32> {
        cci_get!(c_MotController_GetQuadraturePosition(self.handle(), _: i32))
    }
    /**
     * Change the quadrature reported position.
     *
     * Typically this is used to "zero" the sensor.
     * This only works with Quadrature sensor.
     * To set the selected sensor position regardless of what type it is,
     * see `MotorController::set_selected_sensor_position`.
     */
    fn set_quadrature_position(&mut self, new_position: i32, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_SetQuadraturePosition(self.handle(), new_position, timeout_ms) }
    }
    fn quadrature_velocity(&self) -> Result<i32> {
        cci_get!(c_MotController_GetQuadratureVelocity(self.handle(), _: i32))
    }
    fn pulse_width_position(&self) -> Result<i32> {
        cci_get!(c_MotController_GetPulseWidthPosition(self.handle(), _: i32))
    }
    fn set_pulse_width_position(&mut self, new_position: i32, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_SetPulseWidthPosition(self.handle(), new_position, timeout_ms) }
    }
    fn pulse_width_velocity(&self) -> Result<i32> {
        cci_get!(c_MotController_GetPulseWidthVelocity(self.handle(), _: i32))
    }
    fn pulse_width_rise_to_fall_us(&self) -> Result<i32> {
        cci_get!(c_MotController_GetPulseWidthRiseToFallUs(self.handle(), _: i32))
    }
    fn pulse_width_rise_to_rise_us(&self) -> Result<i32> {
        cci_get!(c_MotController_GetPulseWidthRiseToRiseUs(self.handle(), _: i32))
    }
    fn pin_state_quad_a(&self) -> Result<bool> {
        Ok(cci_get!(c_MotController_GetPinStateQuadA(self.handle(), _: i32))? != 0)
    }
    fn pin_state_quad_b(&self) -> Result<bool> {
        Ok(cci_get!(c_MotController_GetPinStateQuadB(self.handle(), _: i32))? != 0)
    }
    fn pin_state_quad_idx(&self) -> Result<bool> {
        Ok(cci_get!(c_MotController_GetPinStateQuadIdx(self.handle(), _: i32))? != 0)
    }
    /// Is forward limit switch closed.
    ///
    /// This function works regardless if limit switch feature is enabled.
    fn is_fwd_limit_switch_closed(&self) -> Result<bool> {
        Ok(cci_get!(c_MotController_IsFwdLimitSwitchClosed(self.handle(), _: i32))? != 0)
    }
    /// Is reverse limit switch closed.
    ///
    /// This function works regardless if limit switch feature is enabled.
    fn is_rev_limit_switch_closed(&self) -> Result<bool> {
        Ok(cci_get!(c_MotController_IsRevLimitSwitchClosed(self.handle(), _: i32))? != 0)
    }
}

/// CTRE Talon SRX Motor Controller when used on CAN Bus.
#[derive(Debug)]
pub struct TalonSRX {
    handle: Handle,
    arb_id: i32,
}

impl TalonSRX {
    /// Constructor.
    ///
    /// # Parameters
    /// - `device_number`: CAN device ID of Talon SRX [0,62]
    pub fn new(device_number: u8) -> Self {
        let arb_id = i32::from(device_number) | 0x0204_0000;
        let handle = unsafe { c_MotController_Create1(arb_id) };
        Self { handle, arb_id }
    }

    /**
     * Inverts the hbridge output of the motor controller.
     *
     * This does not impact sensor phase and should not be used to correct sensor polarity.
     *
     * This will invert the hbridge output but NOT the LEDs.
     * This ensures....
     *  - Green LEDs always represents positive request from robot-controller/closed-looping mode.
     *  - Green LEDs correlates to forward limit switch.
     *  - Green LEDs correlates to forward soft limit.
     */
    pub fn set_inverted(&mut self, invert: impl Into<InvertType>) {
        MotorController::set_inverted(self, invert.into())
    }
}

impl Drop for TalonSRX {
    fn drop(&mut self) {
        unsafe { c_MotController_Destroy(self.handle) };
    }
}

impl MotorController for TalonSRX {
    #[doc(hidden)]
    fn handle(&self) -> Handle {
        self.handle
    }

    #[doc(hidden)]
    fn base_id(&self) -> i32 {
        self.arb_id
    }
}

impl MotorControllerConfig for TalonSRX {
    type PIDSetConfiguration = TalonSRXPIDSetConfiguration;

    fn configure_pid(
        &mut self,
        pid: &Self::PIDSetConfiguration,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> ErrorCode {
        self.base_configure_pid(&pid._base, pid_loop, timeout_ms)
            .or(self.config_selected_feedback_sensor(
                pid.selected_feedback_sensor,
                pid_loop,
                timeout_ms,
            ))
    }

    fn get_pid_configs(
        &self,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> Result<Self::PIDSetConfiguration> {
        let _base = self.base_get_pid_configs(pid_loop, timeout_ms)?;
        let selected_feedback_sensor = f64_to_enum!(self.config_get_parameter(
            ParamEnum::FeedbackSensorType,
            pid_loop as _,
            timeout_ms,
        )? => FeedbackDevice);
        Ok(Self::PIDSetConfiguration {
            _base,
            selected_feedback_sensor,
        })
    }
}

impl ConfigAll for TalonSRX {
    type Configuration = TalonSRXConfiguration;

    fn config_all_settings(
        &mut self,
        all_configs: &Self::Configuration,
        timeout_ms: i32,
    ) -> ErrorCode {
        self.base_config_all_settings(&all_configs._base, timeout_ms)
        // ...
    }

    fn get_all_configs(&self, timeout_ms: i32) -> Result<TalonSRXConfiguration> {
        let _base = self.base_get_all_configs(timeout_ms)?;
        todo!()
    }
}

impl TalonSRX {
    /// Gets the output current of the motor controller in amps.
    pub fn output_current(&self) -> Result<f64> {
        cci_get!(c_MotController_GetOutputCurrent(self.handle, _: f64))
    }

    /// Select the feedback device for the motor controller.
    pub fn config_selected_feedback_sensor(
        &mut self,
        feedback_device: FeedbackDevice,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigSelectedFeedbackSensor(
                self.handle,
                feedback_device as _,
                pid_loop as _,
                timeout_ms,
            )
        }
    }

    /**
     * Select what sensor term should be bound to switch feedback device.
     *
     * - Sensor Sum = Sensor Sum Term 0 - Sensor Sum Term 1
     * - Sensor Difference = Sensor Diff Term 0 - Sensor Diff Term 1
     *
     * The four terms are specified with this routine.  Then Sensor
     * Sum/Difference can be selected for closed-looping.
     */
    pub fn config_sensor_term(
        &mut self,
        sensor_term: SensorTerm,
        feedback_device: FeedbackDevice,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigSensorTerm(
                self.handle,
                sensor_term as _,
                feedback_device as _,
                timeout_ms,
            )
        }
    }

    // XXX: not provided by CTRE's APIs
    /*
    pub fn set_control_frame_period(
        &mut self,
        frame: ControlFrameEnhanced,
        period_ms: i32,
    ) -> ErrorCode {
        unsafe { c_MotController_SetControlFramePeriod(self.handle, frame as _, period_ms) }
    }
    */
    pub fn set_status_frame_period(
        &mut self,
        frame: StatusFrameEnhanced,
        period_ms: u8,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_SetStatusFramePeriod(self.handle, frame as _, period_ms, timeout_ms)
        }
    }
    pub fn get_status_frame_period(
        &self,
        frame: StatusFrameEnhanced,
        timeout_ms: i32,
    ) -> Result<i32> {
        cci_get!(c_MotController_GetStatusFramePeriod(self.handle, frame as _, _: i32, timeout_ms))
    }

    /**
     * Configures the forward limit switch for a local/remote source.
     *
     * For example, a CAN motor controller may need to monitor the Limit-F pin
     * of another Talon, CANifier, or local Gadgeteer feedback connector.
     *
     * If the sensor is remote, a device ID of zero is assumed.
     * If that's not desired, use the four parameter version of this function.
     *
     * # Parameters
     *
     * - `type_`: Limit switch source.
     *   User can choose between the feedback connector, remote Talon SRX, CANifier, or deactivate the feature.
     * - `normal_open_or_close`: Setting for normally open, normally closed, or disabled.
     *   This setting matches the web-based configuration drop down.
     * - `timeout_ms`: Timeout value in ms.
     *   If nonzero, function will wait for config success and report an error if it times out.
     *   If zero, no blocking or checking is performed.
     */
    pub fn config_forward_limit_switch_source(
        &mut self,
        type_: LimitSwitchSource,
        normal_open_or_close: LimitSwitchNormal,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigForwardLimitSwitchSource(
                self.handle,
                type_ as _,
                normal_open_or_close as _,
                0,
                timeout_ms,
            )
        }
    }
    /**
     * Configures the reverse limit switch for a local/remote source.
     *
     * For example, a CAN motor controller may need to monitor the Limit-R pin
     * of another Talon, CANifier, or local Gadgeteer feedback connector.
     *
     * If the sensor is remote, a device ID of zero is assumed.
     * If that's not desired, use the four parameter version of this function.
     *
     * # Parameters
     *
     * - `type_`: Limit switch source.
     *   User can choose between the feedback connector, remote Talon SRX, CANifier, or deactivate the feature.
     * - `normal_open_or_close`: Setting for normally open, normally closed, or disabled.
     *   This setting matches the web-based configuration drop down.
     * - `timeout_ms`: Timeout value in ms.
     *   If nonzero, function will wait for config success and report an error if it times out.
     *   If zero, no blocking or checking is performed.
     */
    pub fn config_reverse_limit_switch_source(
        &mut self,
        type_: LimitSwitchSource,
        normal_open_or_close: LimitSwitchNormal,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe {
            c_MotController_ConfigReverseLimitSwitchSource(
                self.handle,
                type_ as _,
                normal_open_or_close as _,
                0,
                timeout_ms,
            )
        }
    }

    /**
     * Configure the peak allowable current (when current limit is enabled).
     *
     * Current limit is activated when current exceeds the peak limit for longer
     * than the peak duration. Then software will limit to the continuous limit.
     * This ensures current limiting while allowing for momentary excess current
     * events.
     *
     * For simpler current-limiting (single threshold) use
     * [`config_continuous_current_limit`] and set the peak to zero:
     * `config_peak_current_limit(0)`.
     *
     * [`config_continuous_current_limit`]: #method.config_continuous_current_limit
     */
    pub fn config_peak_current_limit(&mut self, amps: i32, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_ConfigPeakCurrentLimit(self.handle, amps, timeout_ms) }
    }
    /**
     * Configure the peak allowable duration (when current limit is enabled).
     *
     * Current limit is activated when current exceeds the peak limit for longer
     * than the peak duration. Then software will limit to the continuous limit.
     * This ensures current limiting while allowing for momentary excess current
     * events.
     *
     * For simpler current-limiting (single threshold) use
     * [`config_continuous_current_limit`] and set the peak to zero:
     * `config_peak_current_limit(0)`.
     *
     * # Parameters
     *
     * - `milliseconds`: How long to allow current-draw past peak limit.
     * - `timeout_ms`: Timeout value in ms.
     *   If nonzero, function will wait for config success and report an error if it times out.
     *   If zero, no blocking or checking is performed.
     *
     * [`config_continuous_current_limit`]: #method.config_continuous_current_limit
     */
    pub fn config_peak_current_duration(
        &mut self,
        milliseconds: i32,
        timeout_ms: i32,
    ) -> ErrorCode {
        unsafe { c_MotController_ConfigPeakCurrentLimit(self.handle, milliseconds, timeout_ms) }
    }
    /**
     * Configure the continuous allowable current-draw (when current limit is enabled).
     *
     * Current limit is activated when current exceeds the peak limit for longer
     * than the peak duration. Then software will limit to the continuous limit.
     * This ensures current limiting while allowing for momentary excess current
     * events.
     *
     * For simpler current-limiting (single threshold) use
     * `config_continuous_current_limit()` and set the peak to zero:
     * `config_peak_current_limit(0)`.
     */
    pub fn config_continuous_current_limit(&mut self, amps: i32, timeout_ms: i32) -> ErrorCode {
        unsafe { c_MotController_ConfigContinuousCurrentLimit(self.handle, amps, timeout_ms) }
    }
    pub fn enable_current_limit(&mut self, enable: bool) {
        unsafe { c_MotController_EnableCurrentLimit(self.handle, enable) };
    }
}

impl SensorCollection for TalonSRX {}

/// VEX Victor SPX Motor Controller when used on CAN Bus.
#[derive(Debug)]
pub struct VictorSPX {
    handle: Handle,
    arb_id: i32,
}

impl VictorSPX {
    /// Constructor.
    ///
    /// # Parameters
    /// - `device_number`: [0,62]
    pub fn new(device_number: u8) -> Self {
        let arb_id = i32::from(device_number) | 0x0104_0000;
        let handle = unsafe { c_MotController_Create1(arb_id) };
        Self { handle, arb_id }
    }

    /**
     * Inverts the hbridge output of the motor controller.
     *
     * This does not impact sensor phase and should not be used to correct sensor polarity.
     *
     * This will invert the hbridge output but NOT the LEDs.
     * This ensures....
     *  - Green LEDs always represents positive request from robot-controller/closed-looping mode.
     *  - Green LEDs correlates to forward limit switch.
     *  - Green LEDs correlates to forward soft limit.
     */
    pub fn set_inverted(&mut self, invert: impl Into<InvertType>) {
        MotorController::set_inverted(self, invert.into())
    }
}

impl Drop for VictorSPX {
    fn drop(&mut self) {
        unsafe { c_MotController_Destroy(self.handle) };
    }
}

impl MotorController for VictorSPX {
    #[doc(hidden)]
    fn handle(&self) -> Handle {
        self.handle
    }

    #[doc(hidden)]
    fn base_id(&self) -> i32 {
        self.arb_id
    }
}

impl MotorControllerConfig for VictorSPX {
    type PIDSetConfiguration = VictorSPXPIDSetConfiguration;

    fn configure_pid(
        &mut self,
        pid: &Self::PIDSetConfiguration,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> ErrorCode {
        self.base_configure_pid(&pid._base, pid_loop, timeout_ms)
            .or(self.config_selected_feedback_sensor(
                pid.selected_feedback_sensor,
                pid_loop,
                timeout_ms,
            ))
    }

    fn get_pid_configs(
        &self,
        pid_loop: PIDLoop,
        timeout_ms: i32,
    ) -> Result<Self::PIDSetConfiguration> {
        let _base = self.base_get_pid_configs(pid_loop, timeout_ms)?;
        let selected_feedback_sensor = f64_to_enum!(self.config_get_parameter(
            ParamEnum::FeedbackSensorType,
            pid_loop as _,
            timeout_ms,
        )? => RemoteFeedbackDevice);
        Ok(Self::PIDSetConfiguration {
            _base,
            selected_feedback_sensor,
        })
    }
}

impl ConfigAll for VictorSPX {
    type Configuration = VictorSPXConfiguration;

    fn config_all_settings(
        &mut self,
        all_configs: &Self::Configuration,
        timeout_ms: i32,
    ) -> ErrorCode {
        self.base_config_all_settings(&all_configs._base, timeout_ms)
        // ...
    }

    fn get_all_configs(&self, timeout_ms: i32) -> Result<VictorSPXConfiguration> {
        let _base = self.base_get_all_configs(timeout_ms)?;
        todo!()
    }
}

// Prevent users from implementing the MotorController trait.
mod private {
    pub trait Sealed {}
    impl Sealed for super::TalonSRX {}
    impl Sealed for super::VictorSPX {}
}
