//! `bxt_force_emit_viewentity_player`

use super::Module;
use crate::hooks::engine;
use crate::modules::cvars::CVar;
use crate::utils::*;

pub struct ForceEmitViewentityPlayer;
impl Module for ForceEmitViewentityPlayer {
    fn name(&self) -> &'static str {
        "bxt_force_emit_viewentity_player"
    }

    fn description(&self) -> &'static str {
        "Forces the engine to add the viewentity player to the list of visible entities every frame"
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_FORCE_EMIT_VIEWENTITY_PLAYER];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::CL_LinkPlayers.is_set(marker)
            && engine::ClientDLL_IsThirdPerson.is_set(marker)
            && engine::cl.is_set(marker)
            && engine::cl_entities.is_set(marker)
            && engine::currententity.is_set(marker)
    }
}

static BXT_FORCE_EMIT_VIEWENTITY_PLAYER: CVar = CVar::new(
    b"bxt_force_emit_viewentity_player\0",
    b"0\0",
    "Set to `1` to force the engine to add the viewentity player to the list of visible entities every frame",
);

static SHOULD_OVERRIDE_CLIENTDLL_ISTHIRDPERSON: MainThreadRefCell<bool> =
    MainThreadRefCell::new(false);

pub fn on_before_cl_linkplayers(marker: MainThreadMarker) {
    if !ForceEmitViewentityPlayer.is_enabled(marker) {
        return;
    }

    if !BXT_FORCE_EMIT_VIEWENTITY_PLAYER.as_bool(marker) {
        return;
    }

    *SHOULD_OVERRIDE_CLIENTDLL_ISTHIRDPERSON.borrow_mut(marker) = true;
}

pub fn on_after_cl_linkplayers(marker: MainThreadMarker) {
    if !ForceEmitViewentityPlayer.is_enabled(marker) {
        return;
    }

    *SHOULD_OVERRIDE_CLIENTDLL_ISTHIRDPERSON.borrow_mut(marker) = false;
}

pub fn should_override_clientdll_isthirdperson(marker: MainThreadMarker) -> bool {
    if !ForceEmitViewentityPlayer.is_enabled(marker) {
        return false;
    }

    *SHOULD_OVERRIDE_CLIENTDLL_ISTHIRDPERSON.borrow(marker)
}

static VIEWMODEL_ATTACHMENTS: MainThreadRefCell<[[f32; 3]; 4]> =
    MainThreadRefCell::new([[0.0; 3]; 4]);

pub unsafe fn save_viewmodel_attachments(marker: MainThreadMarker) {
    if !ForceEmitViewentityPlayer.is_enabled(marker) {
        return;
    }

    if !BXT_FORCE_EMIT_VIEWENTITY_PLAYER.as_bool(marker) {
        return;
    }

    let cl = &*engine::cl.get(marker);
    let cl_entities = &*engine::cl_entities.get(marker);
    let entity = &*cl_entities.add(cl.viewentity as usize);

    *VIEWMODEL_ATTACHMENTS.borrow_mut(marker) = entity.attachment;
}

pub unsafe fn load_viewmodel_attachments(marker: MainThreadMarker) {
    if !ForceEmitViewentityPlayer.is_enabled(marker) {
        return;
    }

    if !BXT_FORCE_EMIT_VIEWENTITY_PLAYER.as_bool(marker) {
        return;
    }

    let cl = &*engine::cl.get(marker);
    let cl_entities = &*engine::cl_entities.get(marker);
    let entity = &mut *cl_entities.add(cl.viewentity as usize);

    entity.attachment = *VIEWMODEL_ATTACHMENTS.borrow(marker);
}

// TODO: add option for this (maybe you want events from both, or just from player model)
pub unsafe fn should_skip_studio_events(marker: MainThreadMarker) -> bool {
    if !ForceEmitViewentityPlayer.is_enabled(marker) {
        return false;
    }

    if !BXT_FORCE_EMIT_VIEWENTITY_PLAYER.as_bool(marker) {
        return false;
    }

    let cl = &*engine::cl.get(marker);
    let cl_entities = &*engine::cl_entities.get(marker);
    let entity = cl_entities.add(cl.viewentity as usize);

    let currententity = *engine::currententity.get(marker);
    if currententity == entity {
        return true;
    }

    false
}
