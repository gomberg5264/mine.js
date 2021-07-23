use specs::{ReadExpect, ReadStorage, System, WriteStorage};

use server_utils::raycast;

use server_common::{math::approx_equals, vec::Vec3};

use crate::{
    comp::{
        lookat::{LookAt, LookTarget},
        rigidbody::RigidBody,
        view_radius::ViewRadius,
    },
    engine::{chunks::Chunks, kdtree::KdTree},
};

pub struct ObserveSystem;

impl<'a> System<'a> for ObserveSystem {
    #[allow(clippy::type_complexity)]
    type SystemData = (
        ReadExpect<'a, KdTree>,
        ReadExpect<'a, Chunks>,
        ReadStorage<'a, RigidBody>,
        ReadStorage<'a, ViewRadius>,
        WriteStorage<'a, LookAt>,
    );

    fn run(&mut self, data: Self::SystemData) {
        use specs::Join;

        let (tree, chunks, bodies, radiuses, mut look_ats) = data;

        let dimension = chunks.config.dimension;
        let test_solid = |x: i32, y: i32, z: i32| -> bool { chunks.get_solidity_by_voxel(x, y, z) };

        for (body, radius, look_at) in (&bodies, &radiuses, &mut look_ats).join() {
            let mut position = body.get_head_position();

            // loop until found or nothing found
            let mut closest: Option<Vec3<f32>> = None;

            let mut offset = 0;
            let mut count = 1;

            loop {
                let closest_arr = (match look_at.0 {
                    LookTarget::ALL(_) => tree.search(&position, count),
                    LookTarget::ENTITY(_) => tree.search_entity(&position, count, true),
                    LookTarget::PLAYER(_) => tree.search_player(&position, count, false),
                })
                .into_iter()
                .map(|(_, entity)| bodies.get(entity.to_owned()).unwrap().get_head_position())
                .collect::<Vec<_>>();

                if closest_arr.is_empty() || closest_arr.len() < count {
                    break;
                }

                closest = if closest_arr.is_empty() {
                    continue;
                } else {
                    Some(closest_arr[offset].clone())
                };

                // check if there are any blocks in between
                if let Some(c) = &closest {
                    let mut dir = c.clone().sub(&position);
                    let dist = dir.len();

                    // closest point is too far, target nothing
                    if dist > radius.0 as f32 * dimension as f32 {
                        closest = None;
                        break;
                    }

                    if !approx_equals(&dist, &0.0) {
                        // there's something blocking the target from seeing
                        let hit = raycast::trace(
                            dist,
                            &test_solid,
                            &mut position,
                            &mut dir,
                            &mut Vec3::default(),
                            &mut Vec3::default(),
                        );

                        if hit {
                            offset += 1;
                            count += 1;
                            closest = None;
                            continue;
                        }
                    }
                }

                if closest.is_some() {
                    break;
                }
            }

            look_at.0 = LookTarget::insert(&look_at.0, closest);
        }
    }
}
