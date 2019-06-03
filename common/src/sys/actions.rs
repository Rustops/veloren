use crate::{comp::Attacking, state::DeltaTime};
use specs::{Entities, Join, Read, System, WriteStorage};

// Basic ECS AI agent system
pub struct Sys;

impl<'a> System<'a> for Sys {
    type SystemData = (
        Entities<'a>,
        Read<'a, DeltaTime>,
        WriteStorage<'a, Attacking,>,
        WriteStorage<'a, Rolling,>, 


    );

    fn run(&mut self, (entities, dt, mut attacks, mut rolls): Self::SystemData) {
        for (entity, attack) in (&entities, &mut attacks,).join() {
            attack.time += dt.0;
        }
        let finished_attacks = (&entities, &mut attacks)
            .join()
            .filter(|(_, a)| a.time > 0.25) // TODO: constant
            .map(|(e, _)| e)
            .collect::<Vec<_>>();

        for entity in finished_attacks {
            attacks.remove(entity);
        }
<<<<<<< HEAD
            roll.time += dt.0;
=======
        for (entity, roll) in (&entities, &mut rolls).join() {

            roll.time += dt.0;
		} 
>>>>>>> fixed timing bug
        let finished_rolls = (&entities, &mut rolls)
            .join()
            .filter(|(e, a)| {
                a.time > 0.25 // TODO: constant
            })
            .map(|(e, a)| e)
            .collect::<Vec<_>>();

        for entity in finished_rolls {
            rolls.remove(entity);
        }
    }
}
