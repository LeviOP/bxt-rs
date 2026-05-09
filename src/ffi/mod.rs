//! Raw bindings to C structs and functions.
//!
//! These are generated with a command similar to:
//!
//! ```sh
//! bindgen /path/to/halflife/pm_shared/pm_defs.h --whitelist-type playermove_s -- --target=i686-unknown-linux-gnu -I/path/to/halflife/{public,common} -include mathlib.h -include const.h > src/ffi/playermove.rs
//! ```
//!
//! and then manually cleaned up a bit.

pub mod beamdef;
pub mod buttons;
pub mod cl_entity;
pub mod com_model;
pub mod command;
pub mod crc;
pub mod cvar;
pub mod dlight;
pub mod edict;
pub mod entity_state;
pub mod particledef;
pub mod physent;
pub mod playermove;
pub mod pmplane;
pub mod pmtrace;
pub mod progs;
pub mod r_efx;
pub mod triangleapi;
pub mod usercmd;
pub mod weaponinfo;
