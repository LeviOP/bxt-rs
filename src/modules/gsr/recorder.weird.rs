use bxt_macros::Recordable;
// use bytemuck::{bytes_of, cast_slice};
use color_eyre::eyre;

use std::{fs::File, io::{BufWriter, Write}};

use crate::{hooks::engine, utils::MainThreadMarker};

pub enum Message<'a> {
    NewObject(ObjectType, Vec<Change<'a>>),
    UpdateObject(ObjectType, Vec<Change<'a>>),
}

pub enum ObjectField {
    Origin,
    Angles,
    Fov,
}

// impl ObjectField {
//     pub const fn size(&self) -> usize {
//         match self {
//             ObjectField::Origin => std::mem::size_of::<[f32; 3]>(),
//             ObjectField::Angles => std::mem::size_of::<[f32; 3]>(),
//             ObjectField::Fov => std::mem::size_of::<f32>(),
//         }
//     }
// }
//
pub enum ObjectType {
    Camera,
    Entity,
}

pub struct Change<'a> {
    pub field: ObjectField,
    pub bytes: &'a [u8],
}

// #[derive(Recordable)]
pub struct Camera {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub fov: f32,
}
impl Camera {
    pub fn get_changes(&self, current: &Camera, changes: &mut Vec<Change>) -> eyre::Result<()> {
        if self.origin != current.origin {
            changes.push(Change {
                field: ObjectField::Origin,
                bytes: bytemuck::bytes_of(&current.origin),
            });
        }
        if self.angles != current.angles {
            changes.push(Change {
                field: ObjectField::Angles,
                bytes: bytemuck::bytes_of(&current.angles),
            });
        }
        if self.fov != current.fov {
            changes.push(Change {
                field: ObjectField::Fov,
                bytes: bytemuck::bytes_of(&current.fov),
            });
        }

        Ok(())
    }
    // pub fn write_changes<W: std::io::Write>(&self, writer: &mut W, current: &Camera) -> eyre::Result<()> {
    //     let mut changes = Vec::new();
    //     if self.origin != current.origin {
    //         changes.push(Change {
    //             field: ObjectField::Origin,
    //             bytes: bytemuck::bytes_of(&current.origin),
    //         });
    //     }
    //     if self.angles != current.angles {
    //         changes.push(Change {
    //             field: ObjectField::Angles,
    //             bytes: bytemuck::bytes_of(&current.angles),
    //         });
    //     }
    //     if self.fov != current.fov {
    //         changes.push(Change {
    //             field: ObjectField::Fov,
    //             bytes: bytemuck::bytes_of(&current.fov),
    //         });
    //     }
    //     let change_count = changes.len();
    //     if change_count != 0 {
    //         writer.write_all(&[MessageType::UpdateObject as u8])?;
    //         writer.write_all(&[ObjectType::Camera as u8])?;
    //         writer.write_all(&[change_count as u8])?;
    //         for change in changes {
    //             writer.write_all(&[change.field as u8])?;
    //             writer.write_all(change.bytes)?;
    //         }
    //     }
    //     Ok(())
    // }
}

// impl Camera {
//     pub fn write_new<W: Write>(&self, writer: &mut W) -> eyre::Result<()> {
//         writer.write_all(&[MessageType::NewObject as u8])?;
//         writer.write_all(&[ObjectType::Camera as u8])?;
//         writer.write_all(&[ObjectField::Origin as u8])?;
//         writer.write_all(cast_slice(&self.origin))?;
//         writer.write_all(&[ObjectField::Angles as u8])?;
//         writer.write_all(cast_slice(&self.angles))?;
//         writer.write_all(&[ObjectField::Fov as u8])?;
//         writer.write_all(bytes_of(&self.fov))?;
//         Ok(())
//     }
// }

pub struct Recorder {
    pub writer: BufWriter<File>,

    pub last_camera: Option<Camera>,
}

impl Recorder {
    pub fn init(filename: &String) -> eyre::Result<Recorder> {
        // maybe use OpenOptions
        let writer = BufWriter::new(File::create(filename)?);

        Ok(Recorder {
            writer,
            last_camera: None,
        })
    }

    pub unsafe fn record_frame(&mut self, marker: MainThreadMarker) -> eyre::Result<()> {
        let messages = Vec::new();

        let camera_origin = *engine::r_refdef_vieworg.get(marker);
        let camera_angles = *engine::r_refdef_viewangles.get(marker);
        let camera_fov = *engine::scr_fov_value.get(marker);

        let current_camera = Camera {
            origin: camera_origin,
            angles: camera_angles,
            fov: camera_fov,
        };

        if let Some(last_camera) = self.last_camera.as_mut() {
            last_camera.get_changes(&current_camera)?;
        } else {
            current_camera.write_new(&mut self.writer)?;
            println!("new camera");
        }

        self.last_camera = Some(current_camera);

        self.writer.flush()?;
        Ok(())

        // let mut active_beams = ACTIVE_BEAMS.borrow_mut(marker);
        // let mut seen_beams: HashSet<*mut BEAM> = HashSet::with_capacity(active_beams.len());
        //
        // let mut p_beam = *engine::gpActiveBeams.get(marker);
        // while !p_beam.is_null() {
        //     if active_beams.insert(p_beam) {
        //         // println!("{:?}", *p_beam);
        //     }
        //     seen_beams.insert(p_beam);
        //
        //     p_beam = (*p_beam).next;
        // }
        //
        // active_beams.retain(|p_beam| {
        //     let keep = seen_beams.contains(p_beam);
        //     if !keep {
        //         // println!("beam died");
        //     }
        //     keep
        // });
        //
        // // let gTempEnts: = &*engine::gTempEnts.get(marker);
        // //
        // // let gTempEnts_start = gTempEnts.as_ptr() as usize;
        // // let gTempEnts_end = gTempEnts_start + std::mem::size_of_val(gTempEnts);
        //
        // // for temp_ent in &gTempEnts[0..5] {
        // //     let seen_entity_addr = (&(*temp_ent).entity as *const cl_entity_t) as usize;
        // //     let inside = seen_entity_addr >= gTempEnts_start && seen_entity_addr < gTempEnts_end;
        // //     println!("{}, {}, {}", gTempEnts_start, seen_entity_addr, inside);
        // //     println!("temp ent: {}", temp_ent.die);
        // // }
        //
        // // let mut active_temp_ents = ACTIVE_TEMP_ENTS.borrow_mut(marker);
        // // let mut seen_temp_ents: HashSet<*mut TEMPENTITY> = HashSet::with_capacity(active_temp_ents.len());
        // //
        // // let mut temp_ent = *engine::gpTempEntActive.get(marker);
        // // while !temp_ent.is_null() {
        // //     let seen_entity_addr = (&(*temp_ent).entity as *const cl_entity_t) as usize;
        // //     let inside = seen_entity_addr >= gTempEnts_start && seen_entity_addr < gTempEnts_end;
        // //     println!("IN OTHER: {}, {}, {}", gTempEnts_start, seen_entity_addr, inside);
        // //     if active_temp_ents.insert(temp_ent) {
        // //         println!("new temp!: {:?}", *temp_ent);
        // //     }
        // //     seen_temp_ents.insert(temp_ent);
        // //
        // //     temp_ent = (*temp_ent).next;
        // // }
        // //
        // // active_temp_ents.retain(|temp_ent| {
        // //     let keep = seen_temp_ents.contains(temp_ent);
        // //     if !keep {
        // //         println!("temp ent removed!!!");
        // //     }
        // //     keep
        // // });
        //
        // let cl = &*engine::cl.get(marker);
        // let cl_time = cl.time as f32;
        //
        // let mut active_dlights = ACTIVE_DLIGHTS.borrow_mut(marker);
        //
        // // doing it by index maybe makes sense? but not currently. revisit
        // let cl_dlights = &*engine::cl_dlights.get(marker);
        // for i in 0..cl_dlights.len() {
        //     let dlight = &cl_dlights[i];
        //     if dlight.die < cl_time || dlight.radius == 0. {
        //         if active_dlights.remove(&i) {
        //             // println!("dlight died");
        //         }
        //     } else {
        //         if active_dlights.insert(i) {
        //             // println!("new dlight: {:?}", dlight);
        //         }
        //     }
        // }
        //
        // let mut active_elights = ACTIVE_ELIGHTS.borrow_mut(marker);
        //
        // let cl_elights = &*engine::cl_elights.get(marker);
        // for i in 0..cl_elights.len() {
        //     let elight = &cl_elights[i];
        //     if elight.die < cl_time || elight.radius == 0. {
        //         if active_elights.remove(&i) {
        //             // println!("elight died");
        //         }
        //     } else {
        //         if active_elights.insert(i) {
        //             // println!("new elight: {:?}", elight);
        //         }
        //     }
        // }
        //
        // let mut decals = DECALS.borrow_mut(marker);
        //
        // let worldmodel = &*cl.worldmodel;
        // let surfaces = std::slice::from_raw_parts(worldmodel.surfaces, worldmodel.numsurfaces as usize);
        //
        // for surface in surfaces {
        //     let mut pdecal = surface.pdecals;
        //     while !pdecal.is_null() {
        //         if !decals.contains(&pdecal) {
        //             // println!("new decal! {:?}", *pdecal);
        //             decals.insert(pdecal);
        //         }
        //
        //         pdecal = (*pdecal).pnext;
        //     }
        // }
        //
        // let gTempEnts = &*engine::gTempEnts.get(marker);
        // // maybe calc only once? very very small
        // let gTempEnts_start = gTempEnts.as_ptr() as usize;
        // let gTempEnts_end = gTempEnts_start + std::mem::size_of_val(gTempEnts);
        //
        // let mut entities = ENTITIES.borrow_mut(marker);
        // let mut visible_entities = VISIBLE_ENTITIES.borrow_mut(marker);
        // let mut seen_entities: HashSet<*mut cl_entity_t> = HashSet::with_capacity(visible_entities.len());
        //
        // let cl_numvisedicts = *engine::cl_numvisedicts.get(marker) as usize;
        // let cl_visedicts = &*engine::cl_visedicts.get(marker);
        // for visible_entity in &cl_visedicts[0..cl_numvisedicts] {
        //     seen_entities.insert(*visible_entity);
        //
        //     let entity = &**visible_entity;
        //     match entities.get_mut(visible_entity) {
        //         Some(prev_entity) => {
        //             if !visible_entities.contains(visible_entity) {
        //                 if prev_entity.model_index != entity.curstate.modelindex {
        //                     let new_entity = Entity {
        //                         draw: true,
        //                         model_index: entity.curstate.modelindex,
        //                         origin: entity.origin,
        //                         angles: entity.angles,
        //                         sequence: entity.curstate.sequence,
        //                         frame: entity.curstate.frame,
        //                         // framerate: entity.curstate.framerate,
        //                         // animtime: entity.curstate.animtime,
        //                     };
        //                     println!("new entity baseline (model change): {}", entity.index);
        //                     entities.insert(*visible_entity, new_entity);
        //                     visible_entities.insert(*visible_entity);
        //                     continue;
        //                 }
        //                 println!("entity has come back into view :D");
        //                 visible_entities.insert(*visible_entity);
        //                 prev_entity.draw = true;
        //             }
        //             if prev_entity.model_index != entity.curstate.modelindex {
        //                 println!("MODEL INDEX HAS CHANGED!!!!!!!!!!!!!!!! WTF");
        //                 println!("new model index: {}", entity.curstate.modelindex);
        //                 prev_entity.model_index = entity.curstate.modelindex;
        //             }
        //             if prev_entity.sequence != entity.curstate.sequence {
        //                 println!("new sequence: {}", entity.curstate.sequence);
        //                 prev_entity.sequence = entity.curstate.sequence;
        //             }
        //             if prev_entity.frame != entity.curstate.frame {
        //                 // println!("new frame: {}", entity.curstate.frame);
        //                 prev_entity.frame = entity.curstate.frame;
        //             }
        //             // if prev_entity.framerate != entity.curstate.framerate {
        //             //     prev_entity.framerate = entity.curstate.framerate;
        //             // }
        //             // if prev_entity.animtime != entity.curstate.animtime {
        //             //     prev_entity.animtime = entity.curstate.animtime;
        //             // }
        //             // entity.curstate is not used for temp entities, but it's always
        //             // coppied to entity.* for regular entites, so use that
        //             if prev_entity.origin != entity.origin {
        //                 if entity.player == 1 {
        //                     // println!("player");
        //                 } else {
        //                     println!("new origin: {:?}", entity.origin);
        //                 }
        //                 prev_entity.origin = entity.origin;
        //             }
        //             if prev_entity.angles != entity.angles {
        //                 if entity.player == 1 {
        //                     // println!("player");
        //                 } else {
        //                     println!("new angles: {:?}", entity.angles);
        //                 }
        //                 prev_entity.angles = entity.angles;
        //             }
        //         },
        //         None => {
        //             let new_entity = Entity {
        //                 draw: true,
        //                 model_index: entity.curstate.modelindex,
        //                 origin: entity.origin,
        //                 angles: entity.angles,
        //                 sequence: entity.curstate.sequence,
        //                 frame: entity.curstate.frame,
        //                 // framerate: entity.curstate.framerate,
        //                 // animtime: entity.curstate.animtime,
        //             };
        //
        //             println!("new entity baseline: {}", entity.index);
        //             entities.insert(*visible_entity, new_entity);
        //             visible_entities.insert(*visible_entity);
        //         }
        //     }
        // }
        //
        // visible_entities.retain(|seen_entity| {
        //     let keep = seen_entities.contains(seen_entity);
        //     if !keep {
        //         let entity = entities
        //             .get_mut(seen_entity)
        //             .expect("Visible entity wasn't in ENTITIES hashmap?");
        //         entity.draw = false;
        //         println!("entity no longer visible: {}", entity.model_index);
        //         let seen_entity_addr = *seen_entity as usize;
        //         if seen_entity_addr >= gTempEnts_start && seen_entity_addr < gTempEnts_end {
        //             println!("it was a temp entity, so we're removing it totally.");
        //             entities.remove(seen_entity);
        //         }
        //     }
        //     keep
        // });

        // let packet_entities = &(*cl).frames[(*cl).parsecountmod as usize].packet_entities;
        // let entities = std::slice::from_raw_parts(packet_entities.entities, packet_entities.num_entities as usize);
        //
        // for entity in entities {
        //     println!("{:?}", entity);
        // }
    }
}
