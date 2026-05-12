//! GoldSrc engine State Recording.

use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::sync::LazyLock;

use super::Module;
use crate::ffi::beamdef::BEAM;
use crate::ffi::cl_entity::cl_entity_t;
use crate::ffi::com_model::decal_s;
use crate::ffi::r_efx::TEMPENTITY;
use crate::handler;
use crate::hooks::engine::{self, cactive_t, con_print};
use crate::modules::commands::{self, Command};
use crate::utils::*;

pub struct GSR;
impl Module for GSR {
    fn name(&self) -> &'static str {
        "GoldSrc engine State Recording"
    }

    fn description(&self) -> &'static str {
        "Records the state of the engine every frame"
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[&BXT_GSR_START, &BXT_GSR_STOP, &TEST];
        COMMANDS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        commands::Commands.is_enabled(marker)
            && engine::R_LoadSkys.is_set(marker)
            && engine::movevars.is_set(marker)
            && engine::R_DrawTEntitiesOnList.is_set(marker)
            && engine::numTransObjs.is_set(marker)
            && engine::R_RenderView.is_set(marker)
            && engine::cls.is_set(marker)
            && engine::cl.is_set(marker)
            && engine::Draw_DecalCount.is_set(marker)
            && engine::decal_names.is_set(marker)
            && engine::Draw_DecalIndex.is_set(marker)
            && engine::mod_numknown.is_set(marker)
            && engine::mod_known.is_set(marker)
            && engine::r_refdef_vieworg.is_set(marker)
            && engine::r_refdef_viewangles.is_set(marker)
            && engine::scr_fov_value.is_set(marker)
            && engine::cl_numvisedicts.is_set(marker)
            && engine::cl_visedicts.is_set(marker)
            && engine::transObjects.is_set(marker)
            && engine::gpActiveBeams.is_set(marker)
            && engine::cl_numbeamentities.is_set(marker)
            && engine::cl_beamentities.is_set(marker)
            && engine::gDecalPool.is_set(marker)
            && engine::cl_lightstyle.is_set(marker)
            && engine::CL_Disconnect.is_set(marker)
    }
}

static TEST: Command = Command::new(b"test\0", handler!("test", gsr_test as fn(_)));

fn gsr_test(marker: MainThreadMarker) {
    unsafe {
        let mod_numknown = *engine::mod_numknown.get(marker) as usize;
        let mod_known = &*engine::mod_known.get(marker);
        for model in &mod_known[..mod_numknown] {
            let name = CStr::from_ptr(model.name.as_ptr());
            println!("{}", name.to_str().unwrap());
        }
    }
}

mod recorder;
use recorder::Recorder;

static BXT_GSR_START: Command = Command::new(
    b"bxt_gsr_start\0",
    handler!(
        "bxt_gsr_start [filename.gr]

Starts a GSR recording. The default filename is `recording.gsr`.",
        gsr_start as fn(_),
        gsr_start_with_filename as fn(_, _)
    ),
);

static BXT_GSR_STOP: Command = Command::new(
    b"bxt_gsr_stop\0",
    handler!(
        "bxt_gsr_stop

Stops a GSR recording.",
        gsr_stop as fn(_)
    ),
);

fn gsr_start(marker: MainThreadMarker) {
    gsr_start_with_filename(marker, "game.gsr".to_string());
}

enum State {
    Idle,
    Starting(String),
    Recording(Recorder),
}

static STATE: MainThreadRefCell<State> = MainThreadRefCell::new(State::Idle);

pub fn gsr_start_with_filename(marker: MainThreadMarker, filename: String) {
    if !GSR.is_enabled(marker) {
        return;
    }

    let mut state = STATE.borrow_mut(marker);
    if !matches!(*state, State::Idle) {
        // Already recording.
        return;
    }

    *state = State::Starting(filename);
}

pub fn gsr_stop(marker: MainThreadMarker) {
    let mut state = STATE.borrow_mut(marker);
    if let State::Recording(ref mut _recorder) = *state {
        *state = State::Idle;
    }
}

pub unsafe fn on_cl_disconnect(marker: MainThreadMarker) {
    if !GSR.is_enabled(marker) {
        return;
    }

    gsr_stop(marker);
}

pub unsafe fn record_frame(marker: MainThreadMarker) {
    if !GSR.is_enabled(marker) {
        return;
    }

    let mut state = STATE.borrow_mut(marker);
    if matches!(*state, State::Idle) {
        return;
    }

    let client_state = (*engine::cls.get(marker)).state;
    if client_state != cactive_t::ca_active {
        return;
    }

    if let State::Starting(ref filename) = *state {
        match Recorder::init(marker, filename) {
            Ok(recorder) => *state = State::Recording(recorder),
            Err(err) => {
                error!("error initializing the recorder: {:?}", err);
                con_print(marker, &format!("Error initializing recording: {err}.\n"));
                *state = State::Idle;
                return;
            }
        }
    }

    let recorder = match *state {
        State::Recording(ref mut recorder) => recorder,
        _ => unreachable!(),
    };

    if let Err(err) = recorder.record_frame(marker) {
        eprintln!("Error recording frame: {err:?}");
    }
}

static SKYNAME: MainThreadRefCell<Option<CString>> = MainThreadRefCell::new(None);

pub unsafe fn update_skyname(marker: MainThreadMarker) {
    let movevars = &*engine::movevars.get(marker);
    let mut skyname = SKYNAME.borrow_mut(marker);
    *skyname = Some(CStr::from_ptr(movevars.skyName.as_ptr()).to_owned());
}

static TRANS_OBJ_COUNT: MainThreadRefCell<i32> = MainThreadRefCell::new(0);

pub unsafe fn save_trans_obj_count(marker: MainThreadMarker) {
    *TRANS_OBJ_COUNT.borrow_mut(marker) = *engine::numTransObjs.get(marker);
}
