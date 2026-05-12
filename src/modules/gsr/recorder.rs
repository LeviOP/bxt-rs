use bytemuck::{bytes_of, cast_slice};
use color_eyre::eyre;

use std::{
    collections::HashMap,
    ffi::{c_int, CStr, CString},
    fs::File,
    io::{BufWriter, Write},
};

use crate::{
    ffi::{
        beamdef::{BEAM, FBEAM},
        cl_entity::cl_entity_t,
        com_model::{decal_t, model_s, modtype_t, msurface_t, player_info_s},
        entity_state::color24,
    },
    hooks::engine::{self, lightstyle_t},
    modules::gsr::{self, TRANS_OBJ_COUNT},
    utils::MainThreadMarker,
};

macro_rules! impl_discriminant {
    ($t:ty) => {
        impl $t {
            fn discriminant(&self) -> u8 {
                unsafe { *(self as *const Self as *const u8) }
            }
        }
    };
}

#[repr(u8)]
pub enum Message {
    AddModels(Vec<CString>) = 0,
    NewObject(Object) = 1,
    UpdateObject(Object, Vec<ObjectField>) = 2,
    UpdatePlayer(u8, Vec<PlayerField>) = 3,
    UpdateDecals(Vec<Decal>) = 4,
    UpdateLightstyles(Vec<Lightstyle>) = 5,
    SetView(u32) = 6,
}
impl_discriminant!(Message);

impl Message {
    fn write<W: std::io::Write>(&self, writer: &mut W) -> eyre::Result<()> {
        match self {
            Message::AddModels(model_names) => {
                let model_count = model_names.len() as u16;
                writer.write_all(bytes_of(&model_count))?;
                for model_name in model_names {
                    let bytes = model_name.as_bytes();
                    writer.write_all(&[bytes.len() as u8])?;
                    writer.write_all(bytes)?;
                }
                Ok(())
            }
            Message::NewObject(object) => {
                writer.write_all(&[object.discriminant()])?;
                object.write_new(writer)
            }
            Message::UpdateObject(object, changes) => {
                writer.write_all(&[object.discriminant()])?;
                object.write_update(writer, changes)
            }
            Message::UpdatePlayer(index, changes) => {
                writer.write_all(&[*index])?;
                let change_count = changes.len() as u8;
                writer.write_all(&[change_count])?;
                for field in changes {
                    field.write(writer)?;
                }
                Ok(())
            }
            Message::UpdateDecals(decals) => {
                let decal_count = decals.len() as u16;
                writer.write_all(bytes_of(&decal_count))?;
                for decal in decals {
                    decal.write(writer)?;
                }
                Ok(())
            }
            Message::UpdateLightstyles(lightstyles) => {
                let lightstyle_count = lightstyles.len() as u8; // MAX_LIGHTSTYLES 64
                writer.write_all(bytes_of(&lightstyle_count))?;
                for lightstyle in lightstyles {
                    lightstyle.write(writer)?;
                }
                Ok(())
            }
            Message::SetView(viewentity) => {
                writer.write_all(&bytes_of(viewentity))?;
                Ok(())
            }
        }
    }
}

#[repr(u8)]
pub enum Object {
    Camera(Camera) = 0,
    Entity(Entity) = 1,
    Beam(Beam) = 2,
    ViewEnt(ViewEnt) = 3,
}
impl_discriminant!(Object);

pub trait TrackedObject {
    fn get_tracked_fields(&self) -> Vec<ObjectField>;
    fn write_new<W: std::io::Write>(&self, writer: &mut W) -> eyre::Result<()>;
    fn write_update<W: std::io::Write>(
        &self,
        writer: &mut W,
        changes: &Vec<ObjectField>,
    ) -> eyre::Result<()>;
}

impl TrackedObject for Object {
    fn get_tracked_fields(&self) -> Vec<ObjectField> {
        match self {
            Object::Camera(c) => c.get_tracked_fields(),
            Object::Entity(e) => e.get_tracked_fields(),
            Object::Beam(b) => b.get_tracked_fields(),
            Object::ViewEnt(v) => v.get_tracked_fields(),
        }
    }
    fn write_new<W: std::io::Write>(&self, writer: &mut W) -> eyre::Result<()> {
        match self {
            Object::Camera(c) => c.write_new(writer),
            Object::Entity(e) => e.write_new(writer),
            Object::Beam(b) => b.write_new(writer),
            Object::ViewEnt(v) => v.write_new(writer),
        }
    }
    fn write_update<W: std::io::Write>(
        &self,
        writer: &mut W,
        changes: &Vec<ObjectField>,
    ) -> eyre::Result<()> {
        match self {
            Object::Camera(c) => c.write_update(writer, changes),
            Object::Entity(e) => e.write_update(writer, changes),
            Object::Beam(b) => b.write_update(writer, changes),
            Object::ViewEnt(v) => v.write_update(writer, changes),
        }
    }
}

#[repr(u8)]
pub enum ObjectField {
    Origin([f32; 3]) = 0,
    Angles([f32; 3]) = 1,
    Fov(f32) = 2,
    Draw(bool) = 3,
    Sequence(i32) = 4,
    AnimTime(f32) = 5,
    Framerate(f32) = 6,
    Body(i32) = 7,
    Frame(f32) = 8,
    Scale(f32) = 9,
    RenderMode(i32) = 10,
    RenderAmt(i32) = 11,
    RenderColor([u8; 3]) = 12,
    RenderFx(i32) = 13,
    GaitSequence(i32) = 14,
    MoveType(i32) = 15,
    WeaponModel(Option<u32>) = 16,
    Flags(i32) = 17,
    Delta([f32; 3]) = 18,
    Freq(f32) = 19,
    Width(f32) = 20,
    Amplitude(f32) = 21,
    Speed(f32) = 22,
    Segments(i32) = 23,
    RGB([f32; 3]) = 24,
    Brightness(f32) = 25,
    PrevSequence(i32) = 26,
    SequenceTime(f32) = 27,
    PrevSeqBlending([u8; 2]) = 28,
    PrevAnimTime(f32) = 29,
    PrevOrigin([f32; 3]) = 30,
    PrevAngles([f32; 3]) = 31,
    ModelIndex(u32) = 32,
}
impl_discriminant!(ObjectField);

impl ObjectField {
    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> eyre::Result<()> {
        writer.write_all(&[self.discriminant()])?;
        match self {
            ObjectField::Origin(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Angles(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Fov(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Draw(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Sequence(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::AnimTime(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Framerate(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Body(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Frame(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Scale(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::RenderMode(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::RenderAmt(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::RenderColor(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::RenderFx(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::GaitSequence(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::MoveType(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::WeaponModel(weaponmodel_index) => {
                if let Some(index) = weaponmodel_index {
                    writer.write_all(&[1])?;
                    writer.write_all(bytes_of(index))?;
                } else {
                    writer.write_all(&[0])?;
                }
            }
            ObjectField::Flags(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Delta(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Freq(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Width(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Amplitude(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Speed(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Segments(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::RGB(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::Brightness(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::PrevSequence(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::SequenceTime(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::PrevSeqBlending(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::PrevAnimTime(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::PrevOrigin(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::PrevAngles(v) => {
                writer.write_all(bytes_of(v))?;
            }
            ObjectField::ModelIndex(v) => {
                writer.write_all(bytes_of(v))?;
            }
        }
        Ok(())
    }
}

fn update_field<T, F>(current: &mut T, new: T, make_field: F, fields: &mut Vec<ObjectField>)
where
    T: PartialEq + Copy,
    F: FnOnce(T) -> ObjectField,
{
    if *current != new {
        *current = new;
        fields.push(make_field(new));
    }
}

#[derive(Clone, Copy)]
pub struct Camera {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub fov: f32,
}

impl TrackedObject for Camera {
    fn get_tracked_fields(&self) -> Vec<ObjectField> {
        vec![
            ObjectField::Origin(self.origin),
            ObjectField::Angles(self.angles),
            ObjectField::Fov(self.fov),
        ]
    }
    fn write_new<W: std::io::Write>(&self, writer: &mut W) -> eyre::Result<()> {
        let fields = self.get_tracked_fields();
        writer.write_all(&[fields.len() as u8])?;
        for field in fields {
            field.write(writer)?;
        }
        Ok(())
    }
    fn write_update<W: std::io::Write>(
        &self,
        writer: &mut W,
        changes: &Vec<ObjectField>,
    ) -> eyre::Result<()> {
        writer.write_all(&[changes.len() as u8])?;
        for field in changes {
            field.write(writer)?;
        }
        Ok(())
    }
}

impl Camera {
    pub fn get_changed_fields(&self, current: &Camera) -> Vec<ObjectField> {
        let mut fields = Vec::new();

        if self.origin != current.origin {
            fields.push(ObjectField::Origin(current.origin))
        }

        if self.angles != current.angles {
            fields.push(ObjectField::Angles(current.angles))
        }

        if self.fov != current.fov {
            fields.push(ObjectField::Fov(current.fov))
        }

        fields
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ViewEnt {
    model: *mut model_s,
    model_index: u32,
    draw: bool,
    origin: [f32; 3],
    angles: [f32; 3],
    sequence: i32,
    animtime: f32,
    body: i32,
    rendermode: i32,
    renderamt: i32,
    rendercolor: color24,
    renderfx: i32,
    movetype: i32,
    prevsequence: i32,
    sequencetime: f32,
    prevseqblending: [u8; 2],
    prevanimtime: f32,
    prevorigin: [f32; 3],
    prevangles: [f32; 3],
}

impl TrackedObject for ViewEnt {
    fn get_tracked_fields(&self) -> Vec<ObjectField> {
        vec![
            ObjectField::Origin(self.origin),
            ObjectField::Angles(self.angles),
            ObjectField::Draw(self.draw),
            ObjectField::ModelIndex(self.model_index),
            ObjectField::Sequence(self.sequence),
            ObjectField::AnimTime(self.animtime),
            ObjectField::Body(self.body),
            ObjectField::RenderMode(self.rendermode),
            ObjectField::RenderAmt(self.renderamt),
            ObjectField::RenderColor([self.rendercolor.r, self.rendercolor.g, self.rendercolor.b]),
            ObjectField::RenderFx(self.renderfx),
            ObjectField::MoveType(self.movetype),
            ObjectField::PrevSequence(self.prevsequence),
            ObjectField::SequenceTime(self.sequencetime),
            ObjectField::PrevSeqBlending(self.prevseqblending),
            ObjectField::PrevAnimTime(self.prevanimtime),
            ObjectField::PrevOrigin(self.prevorigin),
            ObjectField::PrevAngles(self.prevangles),
        ]
    }
    fn write_new<W: std::io::Write>(&self, writer: &mut W) -> eyre::Result<()> {
        let fields = self.get_tracked_fields();
        writer.write_all(&[fields.len() as u8])?;
        for field in fields {
            field.write(writer)?;
        }
        Ok(())
    }
    fn write_update<W: std::io::Write>(
        &self,
        writer: &mut W,
        changes: &Vec<ObjectField>,
    ) -> eyre::Result<()> {
        writer.write_all(&[changes.len() as u8])?;
        for field in changes {
            field.write(writer)?;
        }
        Ok(())
    }
}

impl ViewEnt {
    pub fn new(entity: &cl_entity_t, model_index: u32) -> Self {
        ViewEnt {
            model: entity.model,
            model_index,
            draw: true,
            origin: entity.origin,
            angles: entity.angles,
            sequence: entity.curstate.sequence,
            animtime: entity.curstate.animtime,
            body: entity.curstate.body,
            rendermode: entity.curstate.rendermode,
            renderamt: entity.curstate.renderamt,
            rendercolor: entity.curstate.rendercolor,
            renderfx: entity.curstate.renderfx,
            movetype: entity.curstate.movetype,
            prevsequence: entity.latched.prevsequence,
            sequencetime: entity.latched.sequencetime,
            prevseqblending: entity.latched.prevseqblending,
            prevanimtime: entity.latched.prevanimtime,
            prevorigin: entity.latched.prevorigin,
            prevangles: entity.latched.prevangles,
        }
    }
    pub fn get_changes_and_update(&mut self, viewent: &cl_entity_t) -> Vec<ObjectField> {
        let mut fields = Vec::new();

        update_field(
            &mut self.origin,
            viewent.origin,
            ObjectField::Origin,
            &mut fields,
        );
        update_field(
            &mut self.angles,
            viewent.angles,
            ObjectField::Angles,
            &mut fields,
        );
        update_field(
            &mut self.sequence,
            viewent.curstate.sequence,
            ObjectField::Sequence,
            &mut fields,
        );
        update_field(
            &mut self.animtime,
            viewent.curstate.animtime,
            ObjectField::AnimTime,
            &mut fields,
        );
        update_field(
            &mut self.body,
            viewent.curstate.body,
            ObjectField::Body,
            &mut fields,
        );
        update_field(
            &mut self.rendermode,
            viewent.curstate.rendermode,
            ObjectField::RenderMode,
            &mut fields,
        );
        update_field(
            &mut self.renderamt,
            viewent.curstate.renderamt,
            ObjectField::RenderAmt,
            &mut fields,
        );
        update_field(
            &mut self.rendercolor,
            viewent.curstate.rendercolor,
            |c| ObjectField::RenderColor([c.r, c.g, c.b]),
            &mut fields,
        );
        update_field(
            &mut self.renderfx,
            viewent.curstate.renderfx,
            ObjectField::RenderFx,
            &mut fields,
        );
        update_field(
            &mut self.movetype,
            viewent.curstate.movetype,
            ObjectField::MoveType,
            &mut fields,
        );
        update_field(
            &mut self.prevsequence,
            viewent.latched.prevsequence,
            ObjectField::PrevSequence,
            &mut fields,
        );
        update_field(
            &mut self.sequencetime,
            viewent.latched.sequencetime,
            ObjectField::SequenceTime,
            &mut fields,
        );
        update_field(
            &mut self.prevseqblending,
            viewent.latched.prevseqblending,
            ObjectField::PrevSeqBlending,
            &mut fields,
        );
        update_field(
            &mut self.prevanimtime,
            viewent.latched.prevanimtime,
            ObjectField::PrevAnimTime,
            &mut fields,
        );
        update_field(
            &mut self.prevorigin,
            viewent.latched.prevorigin,
            ObjectField::PrevOrigin,
            &mut fields,
        );
        update_field(
            &mut self.prevangles,
            viewent.latched.prevangles,
            ObjectField::PrevAngles,
            &mut fields,
        );

        fields
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Entity {
    seen: bool,
    id: u32,
    model: *mut model_s,
    model_index: usize,
    player_index: u8,
    draw: bool,
    origin: [f32; 3],
    angles: [f32; 3],
    sequence: i32,
    animtime: f32,
    framerate: f32,
    body: i32,
    frame: f32,
    scale: f32,
    rendermode: i32,
    renderamt: i32,
    rendercolor: color24,
    renderfx: i32,
    gaitsequence: i32,
    movetype: i32,
    weaponmodel: i32,
    weaponmodel_index: Option<u32>,
    prevsequence: i32,
    sequencetime: f32,
    prevseqblending: [u8; 2],
    prevanimtime: f32,
    prevorigin: [f32; 3],
    prevangles: [f32; 3],
}

impl TrackedObject for Entity {
    fn get_tracked_fields(&self) -> Vec<ObjectField> {
        vec![
            ObjectField::Origin(self.origin),
            ObjectField::Angles(self.angles),
            ObjectField::Draw(self.draw),
            ObjectField::Sequence(self.sequence),
            ObjectField::AnimTime(self.animtime),
            ObjectField::Framerate(self.framerate),
            ObjectField::Body(self.body),
            ObjectField::Frame(self.frame),
            ObjectField::Scale(self.scale),
            ObjectField::RenderMode(self.rendermode),
            ObjectField::RenderAmt(self.renderamt),
            ObjectField::RenderColor([self.rendercolor.r, self.rendercolor.g, self.rendercolor.b]),
            ObjectField::RenderFx(self.renderfx),
            ObjectField::GaitSequence(self.gaitsequence),
            ObjectField::MoveType(self.movetype),
            ObjectField::WeaponModel(self.weaponmodel_index),
            ObjectField::PrevSequence(self.prevsequence),
            ObjectField::SequenceTime(self.sequencetime),
            ObjectField::PrevSeqBlending(self.prevseqblending),
            ObjectField::PrevAnimTime(self.prevanimtime),
            ObjectField::PrevOrigin(self.prevorigin),
            ObjectField::PrevAngles(self.prevangles),
        ]
    }
    fn write_new<W: std::io::Write>(&self, writer: &mut W) -> eyre::Result<()> {
        writer.write_all(bytes_of(&self.id))?;
        writer.write_all(bytes_of(&(self.model_index as u32)))?;
        writer.write_all(bytes_of(&self.player_index))?;
        let fields = self.get_tracked_fields();
        writer.write_all(&[fields.len() as u8])?;
        for field in fields {
            field.write(writer)?;
        }
        Ok(())
    }
    fn write_update<W: std::io::Write>(
        &self,
        writer: &mut W,
        changes: &Vec<ObjectField>,
    ) -> eyre::Result<()> {
        writer.write_all(bytes_of(&self.id))?;
        writer.write_all(&[changes.len() as u8])?;
        for field in changes {
            field.write(writer)?;
        }
        Ok(())
    }
}

impl Entity {
    pub unsafe fn new(
        marker: MainThreadMarker,
        entity: &cl_entity_t,
        id: u32,
        model_index: usize,
        weaponmodel_index: Option<u32>,
    ) -> Self {
        let origin = if (*entity.model).type_ == modtype_t::mod_sprite && entity.curstate.body != 0
        {
            println!("we're doing that thing!");
            *(engine::R_GetAttachmentPoint.get(marker)(
                entity.curstate.skin as c_int,
                entity.curstate.body,
            ) as *const [f32; 3])
        } else {
            entity.origin
        };

        Entity {
            seen: true,
            id,
            model: entity.model,
            model_index,
            player_index: if entity.player != 0 {
                entity.curstate.number as u8 - 1
            } else {
                255
            },
            draw: true,
            origin,
            angles: entity.angles,
            sequence: entity.curstate.sequence,
            animtime: entity.curstate.animtime,
            framerate: entity.curstate.framerate,
            body: entity.curstate.body,
            frame: entity.curstate.frame,
            scale: entity.curstate.scale,
            rendermode: entity.curstate.rendermode,
            renderamt: entity.curstate.renderamt,
            rendercolor: entity.curstate.rendercolor,
            renderfx: entity.curstate.renderfx,
            gaitsequence: entity.curstate.gaitsequence,
            movetype: entity.curstate.movetype,
            weaponmodel: entity.curstate.weaponmodel,
            weaponmodel_index: weaponmodel_index,
            prevsequence: entity.latched.prevsequence,
            sequencetime: entity.latched.sequencetime,
            prevseqblending: entity.latched.prevseqblending,
            prevanimtime: entity.latched.prevanimtime,
            prevorigin: entity.latched.prevorigin,
            prevangles: entity.latched.prevangles,
        }
    }

    pub unsafe fn get_changes_and_update(
        &mut self,
        marker: MainThreadMarker,
        entity: &cl_entity_t,
    ) -> Vec<ObjectField> {
        let mut fields = Vec::new();

        let origin = if (*entity.model).type_ == modtype_t::mod_sprite && entity.curstate.body != 0
        {
            println!("we're doing that thing!");
            *(engine::R_GetAttachmentPoint.get(marker)(
                entity.curstate.skin as c_int,
                entity.curstate.body,
            ) as *const [f32; 3])
        } else {
            entity.origin
        };

        update_field(&mut self.origin, origin, ObjectField::Origin, &mut fields);
        update_field(
            &mut self.angles,
            entity.angles,
            ObjectField::Angles,
            &mut fields,
        );
        update_field(
            &mut self.sequence,
            entity.curstate.sequence,
            ObjectField::Sequence,
            &mut fields,
        );
        update_field(
            &mut self.animtime,
            entity.curstate.animtime,
            ObjectField::AnimTime,
            &mut fields,
        );
        update_field(
            &mut self.framerate,
            entity.curstate.framerate,
            ObjectField::Framerate,
            &mut fields,
        );
        update_field(
            &mut self.body,
            entity.curstate.body,
            ObjectField::Body,
            &mut fields,
        );
        update_field(
            &mut self.frame,
            entity.curstate.frame,
            ObjectField::Frame,
            &mut fields,
        );
        update_field(
            &mut self.scale,
            entity.curstate.scale,
            ObjectField::Scale,
            &mut fields,
        );
        update_field(
            &mut self.rendermode,
            entity.curstate.rendermode,
            ObjectField::RenderMode,
            &mut fields,
        );
        update_field(
            &mut self.renderamt,
            entity.curstate.renderamt,
            ObjectField::RenderAmt,
            &mut fields,
        );
        update_field(
            &mut self.rendercolor,
            entity.curstate.rendercolor,
            |c| ObjectField::RenderColor([c.r, c.g, c.b]),
            &mut fields,
        );
        update_field(
            &mut self.renderfx,
            entity.curstate.renderfx,
            ObjectField::RenderFx,
            &mut fields,
        );
        update_field(
            &mut self.gaitsequence,
            entity.curstate.gaitsequence,
            ObjectField::GaitSequence,
            &mut fields,
        );
        update_field(
            &mut self.movetype,
            entity.curstate.movetype,
            ObjectField::MoveType,
            &mut fields,
        );
        update_field(
            &mut self.prevsequence,
            entity.latched.prevsequence,
            ObjectField::PrevSequence,
            &mut fields,
        );
        update_field(
            &mut self.sequencetime,
            entity.latched.sequencetime,
            ObjectField::SequenceTime,
            &mut fields,
        );
        update_field(
            &mut self.prevseqblending,
            entity.latched.prevseqblending,
            ObjectField::PrevSeqBlending,
            &mut fields,
        );
        update_field(
            &mut self.prevanimtime,
            entity.latched.prevanimtime,
            ObjectField::PrevAnimTime,
            &mut fields,
        );
        update_field(
            &mut self.prevorigin,
            entity.latched.prevorigin,
            ObjectField::PrevOrigin,
            &mut fields,
        );
        update_field(
            &mut self.prevangles,
            entity.latched.prevangles,
            ObjectField::PrevAngles,
            &mut fields,
        );

        fields
    }
}

#[repr(u8)]
pub enum PlayerField {
    Model([i8; 64]) = 0,
    TopColor(i32) = 1,
    BottomColor(i32) = 2,
}
impl_discriminant!(PlayerField);

impl PlayerField {
    pub fn write<W: std::io::Write>(&self, writer: &mut W) -> eyre::Result<()> {
        writer.write_all(&[self.discriminant()])?;
        match self {
            PlayerField::Model(v) => {
                let length = v.iter().position(|&b| b == 0).unwrap_or(v.len());
                writer.write_all(&[length as u8])?;
                writer.write_all(cast_slice(&v[..length]))?;
            }
            PlayerField::TopColor(v) => {
                writer.write_all(bytes_of(v))?;
            }
            PlayerField::BottomColor(v) => {
                writer.write_all(bytes_of(v))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PlayerInfo {
    model: [i8; 64],
    topcolor: i32,
    bottomcolor: i32,
}

impl PlayerInfo {
    pub fn new() -> Self {
        // Ideally this will match whatever the engine intializes with, and the reader will do the
        // same
        PlayerInfo {
            model: [0; 64],
            topcolor: 0,
            bottomcolor: 0,
        }
    }

    fn get_changes_and_update(&mut self, player: &player_info_s) -> Vec<PlayerField> {
        let mut fields = Vec::new();

        if self.model != player.model {
            self.model = player.model;
            fields.push(PlayerField::Model(self.model));
        }
        if self.topcolor != player.topcolor {
            self.topcolor = player.topcolor;
            fields.push(PlayerField::TopColor(self.topcolor));
        }
        if self.bottomcolor != player.bottomcolor {
            self.bottomcolor = player.bottomcolor;
            fields.push(PlayerField::BottomColor(self.bottomcolor));
        }

        fields
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Beam {
    seen: bool,
    id: u32,
    draw: bool,
    type_: i32,
    model_index: usize,
    flags: FBEAM,
    framerate: f32,
    frame: f32,
    source: [f32; 3],
    delta: [f32; 3],
    freq: f32,
    width: f32,
    amplitude: f32,
    speed: f32,
    segments: i32,
    color: [f32; 3],
    brightness: f32,
}

impl TrackedObject for Beam {
    fn get_tracked_fields(&self) -> Vec<ObjectField> {
        vec![
            ObjectField::Draw(self.draw),
            ObjectField::Flags(self.flags.bits()),
            ObjectField::Framerate(self.framerate),
            ObjectField::Frame(self.frame),
            ObjectField::Origin(self.source),
            ObjectField::Delta(self.delta),
            ObjectField::Freq(self.freq),
            ObjectField::Width(self.width),
            ObjectField::Amplitude(self.amplitude),
            ObjectField::Speed(self.speed),
            ObjectField::Segments(self.segments),
            ObjectField::RGB(self.color),
            ObjectField::Brightness(self.brightness),
        ]
    }
    fn write_new<W: std::io::Write>(&self, writer: &mut W) -> eyre::Result<()> {
        writer.write_all(bytes_of(&self.id))?;
        writer.write_all(bytes_of(&self.type_))?;
        writer.write_all(bytes_of(&(self.model_index as u32)))?;
        let fields = self.get_tracked_fields();
        writer.write_all(&[fields.len() as u8])?;
        for field in fields {
            field.write(writer)?;
        }
        Ok(())
    }
    fn write_update<W: std::io::Write>(
        &self,
        writer: &mut W,
        changes: &Vec<ObjectField>,
    ) -> eyre::Result<()> {
        writer.write_all(bytes_of(&self.id))?;
        writer.write_all(&[changes.len() as u8])?;
        for field in changes {
            field.write(writer)?;
        }
        Ok(())
    }
}

fn vec_subtract(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn vec_length(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

unsafe fn calculate_beam(entity: &cl_entity_t, marker: MainThreadMarker) -> Option<BEAM> {
    let cl = &*engine::cl.get(marker);

    let mut source = entity.origin;
    let mut target = entity.angles;
    let mut delta = vec_subtract(target, source);
    let mut freq = (entity.curstate.animtime as f64 * cl.time) as f32;
    let amplitude = entity.curstate.body as f32 * 0.01;
    let brightness =
        engine::CL_FxBlend.get(marker)(entity as *const cl_entity_t as *mut cl_entity_t) as f32
            / 255.0;

    let mut segments =
        (vec_length(delta) * (if amplitude >= 0.5 { 0.25 } else { 0.075 }) + 3.0) as i32;

    let mut flags = FBEAM::default();

    let mut start_entity: i32 = 0;
    let mut end_entity: i32 = 0;

    let beamfx = entity.curstate.rendermode & 0xF;
    // BEAM_ENTPOINT
    if beamfx == 1 {
        flags = FBEAM::ENDENTITY;
        end_entity = entity.curstate.skin as i32;
    // BEAM_ENTS
    } else if beamfx == 2 {
        flags = FBEAM::ENDENTITY | FBEAM::STARTENTITY;
        start_entity = entity.curstate.sequence;
        end_entity = entity.curstate.skin as i32;
    }

    if entity.curstate.rendermode & 0x10 != 0 {
        flags |= FBEAM::from_bits_retain(0x10);
    }
    if entity.curstate.rendermode & 0x20 != 0 {
        flags |= FBEAM::from_bits_retain(0x20);
    }
    if entity.curstate.rendermode & 0x40 != 0 {
        flags |= FBEAM::from_bits_retain(0x40);
    }
    if entity.curstate.rendermode & 0x80 != 0 {
        flags |= FBEAM::from_bits_retain(0x80);
    }

    let frametime = cl.time - cl.oldtime;

    freq = (freq as f64 + frametime) as f32;

    if flags.intersects(FBEAM::STARTENTITY | FBEAM::ENDENTITY) {
        if flags.contains(FBEAM::STARTENTITY) {
            let ent_ptr = engine::R_GetBeamAttachmentEntity.get(marker)(start_entity);
            if ent_ptr.is_null() {
                return None;
            }
            let ent = &*ent_ptr;
            // also checks for pFollowModel but that's never set for beam entity
            if !ent.model.is_null() {
                let pt = engine::R_BeamGetAttachmentPoint.get(marker)(ent_ptr, start_entity);
                source = *(pt as *const [f32; 3]);
                flags |= FBEAM::STARTVISIBLE;
            } else {
                if flags.contains(FBEAM::FOREVER) {
                    flags.remove(FBEAM::STARTENTITY);
                }
            }
        }

        if flags.contains(FBEAM::ENDENTITY) {
            let ent_ptr = engine::R_GetBeamAttachmentEntity.get(marker)(end_entity);
            if ent_ptr.is_null() {
                return None;
            }
            let ent = &*ent_ptr;
            if ent.model.is_null() {
                if flags.contains(FBEAM::FOREVER) {
                    flags.remove(FBEAM::STARTENTITY);
                }
                return None;
            }

            let pt = engine::R_BeamGetAttachmentPoint.get(marker)(ent_ptr, end_entity);
            target = *(pt as *const [f32; 3]);
            flags |= FBEAM::ENDVISIBLE;
        }

        if flags.contains(FBEAM::STARTENTITY) && !flags.contains(FBEAM::STARTVISIBLE) {
            return None;
        }

        let difference = vec_subtract(target, source);

        if vec_length(difference) > 0.0000001 {
            delta = difference;
        }

        segments = (vec_length(delta) * (if amplitude >= 0.5 { 0.25 } else { 0.075 }) + 3.0) as i32;
    }

    // type is always TE_BEAMPOINTS for beam entities
    if engine::R_BeamCull.get(marker)(
        &source as *const f32 as *mut f32,
        &target as *const f32 as *mut f32,
        0,
    ) == 0
    {
        return None;
    }

    Some(BEAM {
        next: std::ptr::null_mut(),
        type_: 0,
        flags,
        source,
        target,
        delta,
        t: 0.,
        freq,
        die: 0.,
        width: entity.curstate.scale,
        amplitude,
        r: entity.curstate.rendercolor.r as f32 / 255.0,
        g: entity.curstate.rendercolor.g as f32 / 255.0,
        b: entity.curstate.rendercolor.b as f32 / 255.0,
        brightness,
        speed: entity.curstate.animtime,
        frameRate: 0.,
        frame: entity.curstate.frame,
        segments,
        startEntity: start_entity,
        endEntity: end_entity,
        modelIndex: entity.curstate.movetype,
        frameCount: 0,
        pFollowModel: std::ptr::null_mut(),
        particles: std::ptr::null_mut(),
    })
}

// CLARIFY: why can't you change beam type midway?
impl Beam {
    pub fn new(beam: &BEAM, id: u32, model_index: usize) -> Self {
        Beam {
            seen: true,
            id,
            draw: true,
            type_: beam.type_,
            model_index,
            flags: beam.flags,
            framerate: beam.frameRate,
            frame: beam.frame,
            source: beam.source,
            delta: beam.delta,
            freq: beam.freq,
            width: beam.width,
            amplitude: beam.amplitude,
            speed: beam.speed,
            segments: beam.segments,
            color: [beam.r, beam.g, beam.b],
            brightness: beam.brightness,
        }
    }
    pub fn get_changes_and_update(&mut self, beam: &BEAM) -> Vec<ObjectField> {
        let mut fields = Vec::new();

        update_field(
            &mut self.flags,
            beam.flags,
            |f| ObjectField::Flags(f.bits()),
            &mut fields,
        );
        update_field(
            &mut self.framerate,
            beam.frameRate,
            ObjectField::Framerate,
            &mut fields,
        );
        update_field(&mut self.frame, beam.frame, ObjectField::Frame, &mut fields);
        update_field(
            &mut self.source,
            beam.source,
            ObjectField::Origin,
            &mut fields,
        );
        update_field(&mut self.delta, beam.delta, ObjectField::Delta, &mut fields);
        update_field(&mut self.freq, beam.freq, ObjectField::Freq, &mut fields);
        update_field(&mut self.width, beam.width, ObjectField::Width, &mut fields);
        update_field(
            &mut self.amplitude,
            beam.amplitude,
            ObjectField::Amplitude,
            &mut fields,
        );
        update_field(&mut self.speed, beam.speed, ObjectField::Speed, &mut fields);
        update_field(
            &mut self.segments,
            beam.segments,
            ObjectField::Segments,
            &mut fields,
        );
        if self.color[0] != beam.r || self.color[1] != beam.g || self.color[2] != beam.b {
            self.color = [beam.r, beam.g, beam.b];
            fields.push(ObjectField::RGB(self.color));
        }
        update_field(
            &mut self.brightness,
            beam.brightness,
            ObjectField::Brightness,
            &mut fields,
        );

        fields
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Decal {
    index: u16,
    surface: *mut msurface_t,
    face: u16, // MAX_MAP_FACES
    dx: f32,
    dy: f32,
    scale: f32,
    texture: i16,
    decal_index: u16,
    flags: i16,
}

impl Decal {
    pub fn new(decal: &decal_t, index: u16, face: u16, decal_index: u16) -> Self {
        Decal {
            index,
            surface: decal.psurface,
            face,
            dx: decal.dx,
            dy: decal.dy,
            scale: decal.scale,
            texture: decal.texture,
            decal_index,
            flags: decal.flags,
        }
    }

    pub fn update(&mut self, decal: &decal_t) -> bool {
        if self.surface != decal.psurface
            || self.dx != decal.dx
            || self.dy != decal.dy
            || self.scale != decal.scale
            || self.texture != decal.texture
            || self.flags != decal.flags
        {
            self.surface = decal.psurface;
            self.dx = decal.dx;
            self.dy = decal.dy;
            self.scale = decal.scale;
            self.texture = decal.texture;
            self.flags = decal.flags;
            return true;
        }
        return false;
    }

    fn write<W: std::io::Write>(&self, writer: &mut W) -> eyre::Result<()> {
        writer.write_all(bytes_of(&self.index))?;
        writer.write_all(bytes_of(&self.face))?;
        writer.write_all(bytes_of(&self.dx))?;
        writer.write_all(bytes_of(&self.dy))?;
        writer.write_all(bytes_of(&self.scale))?;
        writer.write_all(bytes_of(&self.decal_index))?;
        writer.write_all(bytes_of(&self.flags))?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Lightstyle {
    index: u8,
    map: Vec<u8>,
}

impl Lightstyle {
    pub unsafe fn new(lightstyle: &lightstyle_t, index: u8) -> Self {
        Lightstyle {
            index,
            map: std::slice::from_raw_parts(
                lightstyle.map.as_ptr() as *const u8,
                lightstyle.length as usize,
            )
            .to_vec(),
        }
    }

    unsafe fn update(&mut self, lightstyle: &lightstyle_t) -> bool {
        let len = lightstyle.length as usize;
        let map = std::slice::from_raw_parts(lightstyle.map.as_ptr() as *const u8, len);
        if self.map.len() != len || self.map.as_slice() != map {
            self.map.clear();
            self.map.extend_from_slice(map);
            return true;
        }
        return false;
    }

    fn write<W: std::io::Write>(&self, writer: &mut W) -> eyre::Result<()> {
        writer.write_all(&[self.index])?;
        writer.write_all(&[self.map.len() as u8])?;
        writer.write_all(cast_slice(&self.map.as_slice()))?;

        Ok(())
    }
}

pub struct Recorder {
    pub writer: BufWriter<File>,

    pub model_index_map: HashMap<*const model_s, usize>,
    pub last_mod_numknown: usize,
    pub last_camera: Option<Camera>,
    pub last_viewent: Option<ViewEnt>,
    pub entities: Vec<Entity>,
    pub engine_entities: HashMap<*mut cl_entity_t, usize>,
    pub players: [PlayerInfo; 32],
    pub beams: Vec<Beam>,
    pub engine_beams: HashMap<*mut BEAM, usize>,
    pub beam_entities: Vec<Beam>,
    pub engine_beam_entities: HashMap<*mut cl_entity_t, usize>,
    pub decal_cache_name_index_map: HashMap<i16, u16>,
    pub decals: HashMap<usize, Decal>,
    pub lightstyles: HashMap<usize, Lightstyle>,
    pub last_viewentity: Option<c_int>,
}

impl Recorder {
    pub unsafe fn init(marker: MainThreadMarker, filename: &String) -> eyre::Result<Recorder> {
        // maybe use OpenOptions
        let mut writer = BufWriter::new(File::create(filename)?);

        let cl = &*engine::cl.get(marker);
        let worldmodel = &*cl.worldmodel;
        let map_name = CStr::from_ptr(worldmodel.name.as_ptr()).to_owned();
        let map_name_bytes = map_name.to_bytes();
        writer.write_all(&[map_name_bytes.len() as u8])?;
        writer.write_all(map_name_bytes)?;

        let skyname = gsr::SKYNAME
            .borrow(marker)
            .as_ref()
            .expect("Skyname was None")
            .clone();
        let skyname_bytes = skyname.to_bytes();
        writer.write_all(&[skyname_bytes.len() as u8])?;
        writer.write_all(skyname_bytes)?;

        let get_decal_count = engine::Draw_DecalCount.get(marker);
        let decal_count = get_decal_count() as usize;

        writer.write_all(bytes_of(&(decal_count as u16)))?;
        let decal_names = &*engine::decal_names.get(marker);
        for decal_name in &decal_names[..decal_count] {
            let length = decal_name.iter().position(|&b| b == 0).unwrap();
            writer.write_all(&[length as u8])?;
            writer.write_all(cast_slice(&decal_name[..length]))?;
        }

        let get_decal_index = engine::Draw_DecalIndex.get(marker);

        let mut decal_cache_name_index_map = HashMap::new();
        for i in 0..decal_count {
            let cache_index = get_decal_index(i as i32) as i16;
            decal_cache_name_index_map.insert(cache_index, i as u16);
        }

        Ok(Recorder {
            model_index_map: HashMap::new(),
            last_mod_numknown: 0,
            writer,
            last_camera: None,
            last_viewent: None,
            entities: Vec::new(),
            engine_entities: HashMap::new(),
            players: [PlayerInfo::new(); 32],
            beams: Vec::new(),
            engine_beams: HashMap::new(),
            beam_entities: Vec::new(),
            engine_beam_entities: HashMap::new(),
            decals: HashMap::new(),
            decal_cache_name_index_map,
            lightstyles: HashMap::new(),
            last_viewentity: None,
        })
    }

    pub unsafe fn record_frame(&mut self, marker: MainThreadMarker) -> eyre::Result<()> {
        let mut messages = Vec::new();

        // ============================
        // Models
        // ============================

        let mod_numknown = *engine::mod_numknown.get(marker) as usize;
        if mod_numknown != self.last_mod_numknown {
            let mod_known = &*engine::mod_known.get(marker);
            let model_names: Vec<CString> = mod_known[self.last_mod_numknown..mod_numknown]
                .iter()
                .enumerate()
                .map(|(index, model)| {
                    self.model_index_map
                        .insert(model as *const model_s, self.last_mod_numknown + index);
                    CStr::from_ptr(model.name.as_ptr()).to_owned()
                })
                .collect();

            // for name in &model_names {
            //     println!("{name}");
            // }
            messages.push(Message::AddModels(model_names));

            // println!("mod_numknown change! {} -> {}", self.last_mod_numknown, mod_numknown);
            self.last_mod_numknown = mod_numknown;
        }

        // ============================
        // Viewentity
        // ============================

        let cl = &*engine::cl.get(marker);

        if Some(cl.viewentity) != self.last_viewentity {
            messages.push(Message::SetView(cl.viewentity as u32));
            println!("changing viewentity {}", cl.viewentity);
            self.last_viewentity = Some(cl.viewentity);
        }

        // ============================
        // Camera
        // ============================

        let camera_origin = *engine::r_refdef_vieworg.get(marker);
        let camera_angles = *engine::r_refdef_viewangles.get(marker);
        let camera_fov = *engine::scr_fov_value.get(marker);

        let current_camera = Camera {
            origin: camera_origin,
            angles: camera_angles,
            fov: camera_fov,
        };
        if let Some(last_camera) = self.last_camera.as_mut() {
            let changes = last_camera.get_changed_fields(&current_camera);
            if changes.len() != 0 {
                messages.push(Message::UpdateObject(
                    Object::Camera(current_camera),
                    changes,
                ));
            }
        } else {
            messages.push(Message::NewObject(Object::Camera(current_camera)));
        }

        self.last_camera = Some(current_camera);

        // ============================
        // Players
        // ============================

        for (index, (player, last_player)) in (&cl.players)
            .iter()
            .zip(self.players.iter_mut())
            .enumerate()
        {
            let changes = last_player.get_changes_and_update(player);
            if changes.len() != 0 {
                messages.push(Message::UpdatePlayer(index as u8, changes));
            }
        }

        // ============================
        // Viewent
        // ============================

        let viewent = &cl.viewent;

        'update_viewent: {
            if let Some(last_viewent) = self.last_viewent.as_mut() {
                if cl.stats[0] <= 0 || viewent.model.is_null() {
                    if last_viewent.draw {
                        last_viewent.draw = false;
                        messages.push(Message::UpdateObject(
                            Object::ViewEnt(*last_viewent),
                            vec![ObjectField::Draw(false)],
                        ));
                    }
                    break 'update_viewent;
                } else {
                    let mut changes = last_viewent.get_changes_and_update(viewent);
                    if !last_viewent.draw {
                        last_viewent.draw = true;
                        changes.push(ObjectField::Draw(true));
                    }
                    if last_viewent.model != viewent.model {
                        last_viewent.model = viewent.model;
                        let model_index = *self.model_index_map.get(&(viewent.model as *const model_s)).expect("There was a viewent model index that wasn't in the known model list!") as u32;
                        changes.push(ObjectField::ModelIndex(model_index));
                    }
                    if changes.len() != 0 {
                        messages.push(Message::UpdateObject(
                            Object::ViewEnt(*last_viewent),
                            changes,
                        ));
                    }
                }
            } else if !(cl.stats[0] <= 0 || viewent.model.is_null()) {
                let model_index = *self
                    .model_index_map
                    .get(&(viewent.model as *const model_s))
                    .expect("There was a viewent model index that wasn't in the known model list!")
                    as u32;
                let new_viewent = ViewEnt::new(viewent, model_index);
                messages.push(Message::NewObject(Object::ViewEnt(new_viewent)));
                self.last_viewent = Some(new_viewent);
            }
        }

        // ============================
        // Entities
        // ============================

        for entity in &mut self.entities {
            entity.seen = false;
        }

        let cl_numvisedicts = *engine::cl_numvisedicts.get(marker) as usize;
        let cl_visedicts = &*engine::cl_visedicts.get(marker);

        // muzzle flashes are added directly to the trans object list (with AppendTEntity), so
        // using our current model we have traverse it to retreive the edict
        let trans_obj_count = *TRANS_OBJ_COUNT.borrow(marker);
        let trans_objects_ptr = *engine::transObjects.get(marker);
        let trans_objects = std::slice::from_raw_parts(trans_objects_ptr, trans_obj_count as usize);

        let visible_edict_ptrs = cl_visedicts[..cl_numvisedicts]
            .iter()
            .chain(trans_objects.iter().map(|t| &t.pEnt));

        for visible_edict_ptr in visible_edict_ptrs {
            match self.engine_entities.get(visible_edict_ptr) {
                Some(existing_entity_index) => {
                    let edict = &**visible_edict_ptr;
                    let existing_entity = &mut self.entities[*existing_entity_index];

                    if existing_entity.seen {
                        // edict in transObjects that was already handled by cl_visedicts pass
                        continue;
                    }

                    if existing_entity.model != edict.model {
                        let index = self.entities.len();
                        let model_index = *self.model_index_map.get(&(edict.model as *const model_s)).expect("There was a model pointer in an entity that wasn't in the known model list!");
                        let weaponmodel_index = if edict.curstate.weaponmodel != 0 {
                            let weaponmodel =
                                cl.model_precache[edict.curstate.weaponmodel as usize];
                            Some(*self.model_index_map.get(&(weaponmodel as *const model_s)).expect("There was a weapon model index that wasn't in the known model list!") as u32)
                        } else {
                            None
                        };
                        let new_entity = Entity::new(
                            marker,
                            edict,
                            index as u32,
                            model_index,
                            weaponmodel_index,
                        );
                        self.entities.push(new_entity);
                        self.engine_entities.insert(*visible_edict_ptr, index);
                        messages.push(Message::NewObject(Object::Entity(new_entity)));
                    } else {
                        existing_entity.seen = true;
                        let mut changes = existing_entity.get_changes_and_update(marker, edict);
                        if existing_entity.draw == false {
                            existing_entity.draw = true;
                            changes.push(ObjectField::Draw(true));
                        }
                        // easier to do this check here because we need to access self.model_index_map
                        if existing_entity.weaponmodel != edict.curstate.weaponmodel {
                            let weaponmodel_index = if edict.curstate.weaponmodel != 0 {
                                let weaponmodel =
                                    cl.model_precache[edict.curstate.weaponmodel as usize];
                                Some(*self.model_index_map.get(&(weaponmodel as *const model_s)).expect("There was a weapon model index that wasn't in the known model list!") as u32)
                            } else {
                                None
                            };
                            changes.push(ObjectField::WeaponModel(weaponmodel_index));
                        }
                        if changes.len() != 0 {
                            messages.push(Message::UpdateObject(
                                Object::Entity(*existing_entity),
                                changes,
                            ));
                        }
                    }
                }
                None => {
                    // maybe we should check if there is missing something.. or something
                    let edict = &**visible_edict_ptr;
                    if !edict.model.is_null() {
                        let index = self.entities.len();
                        let model_index = *self.model_index_map.get(&(edict.model as *const model_s)).expect("There was a model pointer in an entity that wasn't in the known model list!");
                        let weaponmodel_index = if edict.curstate.weaponmodel != 0 {
                            let weaponmodel =
                                cl.model_precache[edict.curstate.weaponmodel as usize];
                            Some(*self.model_index_map.get(&(weaponmodel as *const model_s)).expect("There was a weapon model index that wasn't in the known model list!") as u32)
                        } else {
                            None
                        };
                        let new_entity = Entity::new(
                            marker,
                            edict,
                            index as u32,
                            model_index,
                            weaponmodel_index,
                        );
                        self.entities.push(new_entity);
                        self.engine_entities.insert(*visible_edict_ptr, index);
                        messages.push(Message::NewObject(Object::Entity(new_entity)));
                    }
                }
            }
        }

        for entity in &mut self.entities {
            if entity.seen == false && entity.draw {
                entity.draw = false;
                messages.push(Message::UpdateObject(
                    Object::Entity(*entity),
                    vec![ObjectField::Draw(false)],
                ));
            }
        }

        // ============================
        // Beams
        // ============================

        for beam_index in self.engine_beams.values() {
            let beam = &mut self.beams[*beam_index];
            beam.seen = false;
        }

        let mut beam_ptr = *engine::gpActiveBeams.get(marker);
        while !beam_ptr.is_null() {
            let beam = &*beam_ptr;

            match self.engine_beams.get(&beam_ptr) {
                Some(existing_beam_index) => {
                    let existing_beam = &mut self.beams[*existing_beam_index];

                    let model = cl.model_precache[beam.modelIndex as usize];
                    let model_index = *self.model_index_map.get(&(model as *const model_s)).expect(
                        "There was a beam model index that wasn't in the known model list!",
                    );
                    if existing_beam.model_index != model_index {
                        let index = self.beams.len();
                        let new_beam = Beam::new(beam, index as u32, model_index);
                        self.beams.push(new_beam);
                        self.engine_beams.insert(beam_ptr, index);
                        messages.push(Message::NewObject(Object::Beam(new_beam)));
                    } else {
                        existing_beam.seen = true;
                        // really only source, delta, segments, die (for followbeams) and freq ever
                        // change
                        let changes = existing_beam.get_changes_and_update(beam);
                        if changes.len() != 0 {
                            messages
                                .push(Message::UpdateObject(Object::Beam(*existing_beam), changes));
                        }
                    }
                }
                None => {
                    let index = self.beams.len();
                    let model = cl.model_precache[beam.modelIndex as usize];
                    let model_index = *self.model_index_map.get(&(model as *const model_s)).expect(
                        "There was a beam model index that wasn't in the known model list!",
                    );
                    let new_beam = Beam::new(beam, index as u32, model_index);
                    self.beams.push(new_beam);
                    self.engine_beams.insert(beam_ptr, index);
                    messages.push(Message::NewObject(Object::Beam(new_beam)));
                }
            }

            beam_ptr = beam.next;
        }

        for beam in &mut self.beams {
            if beam.seen == false && beam.draw {
                beam.draw = false;
                messages.push(Message::UpdateObject(
                    Object::Beam(*beam),
                    vec![ObjectField::Draw(false)],
                ));
            }
        }
        self.engine_beams
            .retain(|_, beam_index| self.beams[*beam_index].seen);

        // ============================
        // Beam entities
        // ============================

        for beam_entity in &mut self.beam_entities {
            beam_entity.seen = false;
        }

        let cl_numbeamentities = *engine::cl_numbeamentities.get(marker) as usize;
        let cl_beamentities = &*engine::cl_beamentities.get(marker);

        for beam_entity_ptr in &cl_beamentities[..cl_numbeamentities] {
            match self.engine_beam_entities.get(beam_entity_ptr) {
                Some(existing_beam_entity_index) => {
                    let beam_entity = &**beam_entity_ptr;
                    let existing_beam_entity = &mut self.beam_entities[*existing_beam_entity_index];

                    if let Some(beam) = calculate_beam(beam_entity, marker) {
                        let model = cl.model_precache[beam.modelIndex as usize];
                        let model_index =
                            *self.model_index_map.get(&(model as *const model_s)).expect(
                                "There was a beam model index that wasn't in the known model list!",
                            );
                        if existing_beam_entity.model_index != model_index {
                            let index = self.beam_entities.len();
                            let new_beam = Beam::new(&beam, index as u32, model_index);
                            self.beam_entities.push(new_beam);
                            self.engine_beam_entities.insert(*beam_entity_ptr, index);
                            messages.push(Message::NewObject(Object::Beam(new_beam)));
                        } else {
                            existing_beam_entity.seen = true;
                            let mut changes = existing_beam_entity.get_changes_and_update(&beam);
                            if existing_beam_entity.draw == false {
                                existing_beam_entity.draw = true;
                                changes.push(ObjectField::Draw(true));
                            }
                            if changes.len() != 0 {
                                messages.push(Message::UpdateObject(
                                    Object::Beam(*existing_beam_entity),
                                    changes,
                                ));
                            }
                        }
                    }
                }
                None => {
                    let beam_entity = &**beam_entity_ptr;
                    if let Some(beam) = calculate_beam(beam_entity, marker) {
                        let index = self.engine_beam_entities.len();
                        let model_index_raw = beam_entity.curstate.movetype as usize;
                        let model = cl.model_precache[model_index_raw];
                        let model_index = *self.model_index_map.get(&(model as *const model_s)).expect("There was a beam entity model index that wasn't in the known model list!");
                        let new_beam = Beam::new(&beam, index as u32, model_index);
                        self.beam_entities.push(new_beam);
                        self.engine_beam_entities.insert(*beam_entity_ptr, index);
                        messages.push(Message::NewObject(Object::Beam(new_beam)));
                    }
                }
            }
        }

        for beam_entity in &mut self.beam_entities {
            if beam_entity.seen == false && beam_entity.draw {
                beam_entity.draw = false;
                messages.push(Message::UpdateObject(
                    Object::Beam(*beam_entity),
                    vec![ObjectField::Draw(false)],
                ));
            }
        }

        // ============================
        // Decals
        // ============================

        let decals = &*engine::gDecalPool.get(marker);
        let mut changed_decals: Vec<Decal> = Vec::new();

        for (index, decal) in decals.iter().enumerate() {
            if decal.psurface.is_null() {
                continue;
            }

            match self.decals.get_mut(&index) {
                Some(prev_decal) => {
                    let has_changed = prev_decal.update(decal);
                    if has_changed {
                        prev_decal.face = prev_decal
                            .surface
                            .offset_from_unsigned((*cl.worldmodel).surfaces)
                            as u16;
                        if decal.texture >= 0 {
                            prev_decal.decal_index =
                                *self.decal_cache_name_index_map.get(&decal.texture).unwrap();
                        } else {
                            prev_decal.decal_index =
                                *self.decal_cache_name_index_map.get(&0).unwrap();
                        }
                        changed_decals.push(*prev_decal);
                    }
                }
                None => {
                    let face = decal
                        .psurface
                        .offset_from_unsigned((*cl.worldmodel).surfaces)
                        as u16;
                    let decal_index;
                    if decal.texture >= 0 {
                        decal_index = *self.decal_cache_name_index_map.get(&decal.texture).unwrap();
                    } else {
                        decal_index = *self.decal_cache_name_index_map.get(&0).unwrap();
                    }
                    let new_decal = Decal::new(decal, index as u16, face, decal_index);
                    self.decals.insert(index, new_decal);
                    changed_decals.push(new_decal);
                }
            }
        }

        if changed_decals.len() != 0 {
            messages.push(Message::UpdateDecals(changed_decals));
        }

        // ============================
        // Lightstyles
        // ============================

        // FIXME: it might be possible for the server to send a lightstyle
        // with length zero, so we should probably check even zero-length
        // lightstyles if we've seen them be non-zero before

        let cl_lightstyle = &*engine::cl_lightstyle.get(marker);
        let mut changed_lightstyles: Vec<Lightstyle> = Vec::new();

        for (index, lightstyle) in cl_lightstyle.iter().enumerate() {
            if lightstyle.length == 0 {
                continue;
            }

            match self.lightstyles.get_mut(&index) {
                Some(prev_lightstyle) => {
                    let has_changed = prev_lightstyle.update(lightstyle);
                    if has_changed {
                        changed_lightstyles.push(prev_lightstyle.clone());
                    }
                }
                None => {
                    let new_lightstyle = Lightstyle::new(lightstyle, index as u8);
                    changed_lightstyles.push(new_lightstyle.clone());
                    self.lightstyles.insert(index, new_lightstyle);
                }
            }
        }

        if changed_lightstyles.len() != 0 {
            messages.push(Message::UpdateLightstyles(changed_lightstyles));
        }

        let time = cl.time as f64;
        self.writer.write_all(bytes_of(&time))?;

        let message_count: u16 = messages.len() as u16;
        self.writer.write_all(bytes_of(&message_count))?;

        for message in messages {
            // write message type
            self.writer.write_all(&[message.discriminant()])?;

            message.write(&mut self.writer)?;
        }

        self.writer.flush()?;

        Ok(())
    }
}
