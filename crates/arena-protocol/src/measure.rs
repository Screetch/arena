use super::*;
use arena_sim::map::FIRST;
use arena_sim::moves::{Seeded, World, Id, Intents};

#[test]
fn breakdown() {
  let mut world = World::new(FIRST, Seeded::new(11));
  for id in 1..=10u32 { world.add(Id(id), (id % 3 + 1) as u8); }
  for step in 0..300u32 {
    for id in 1..=10u32 {
      let mut keys = [false; 5];
      keys[3] = (step + id * 7) % 80 < 30;
      world.intend(Id(id), Intents { keys, aim: [400.0 + f64::from(id) * 380.0, 1400.0], button: (step + id) % 16 < 8, slot: ((step / 60 + id) % 3 + 1) as u8, role: (id % 3 + 1) as u8 });
    }
    world.frame();
  }
  let f = &world.fighters()[&Id(1)];
  println!("fighter {}", postcard::to_allocvec(f).unwrap().len());
  println!("figure {}", postcard::to_allocvec(f.figure()).unwrap().len());
  println!("shots {} n {}", postcard::to_allocvec(world.shots()).unwrap().len(), world.shots().len());
  println!("stuck {} n {}", postcard::to_allocvec(world.stuck()).unwrap().len(), world.stuck().len());
  println!("state {}", postcard::to_allocvec(world.state()).unwrap().len());
  fn count(c: &arena_sim::clip::Clip) -> usize { 1 + c.parts().iter().map(|p| count(&p.clip)).sum::<usize>() }
  println!("clips in figure {}", count(f.figure()));
  for s in world.stuck().iter().take(3) { println!("stuck one {} clips {}", postcard::to_allocvec(s).unwrap().len(), count(&s.clip)); }
  for s in world.shots().iter().take(5) { println!("shot {:?} {} clips {}", s.effect, postcard::to_allocvec(s).unwrap().len(), count(&s.clip)); }
}
