use std::os::raw;

/* automatically generated by rust-bindgen */

extern "C" {
    pub fn c_FeedEnable(timeoutMs: raw::c_int);
}
extern "C" {
    pub fn c_GetEnableState() -> bool;
}
extern "C" {
    pub fn c_SetTransmitEnable(en: bool);
}
extern "C" {
    pub fn c_GetTransmitEnable() -> bool;
}
extern "C" {
    pub fn c_GetPhoenixVersion() -> raw::c_int;
}
extern "C" {
    pub fn c_LoadPhoenix();
}
extern "C" {
    pub fn c_IoControl(ioControlCode: raw::c_int, ioControlParam: raw::c_longlong) -> raw::c_int;
}
