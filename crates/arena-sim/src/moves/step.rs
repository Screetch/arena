//! The moves of the fighter in one step: its aim, its fall, its run, its jumps, its walls and its
//! ledges, its slope, the probes of its body against the map, and the pose of its body.

use std::f64::consts::PI;

use libm::{atan2, cos, sin};

use super::{
  At, Chance, Effect, Key, Pose, RAD, REACH, SIDE, World, approach, arm_mut, body, body_mut, round,
};

/// What the probes of the sides of the body found: the first row of probes that touches a solid,
/// or 4, and whether it touches on the left or on the right.
struct Sides {
  row: u32,
  left: bool,
  right: bool,
}

impl<C: Chance> World<C> {
  pub(super) fn step(&mut self) {
    self.start();
    if self.fighter.stunned == 0 {
      self.steer();
    }
    self.coast();
    let sides = self.collide();
    self.cling(&sides);
    self.finish();
  }

  fn held(&self, key: Key) -> bool {
    self.held[key as usize]
  }

  fn aim(&mut self) {
    let fighter = &self.fighter;
    let t = (fighter.slope + 90.0) * RAD;
    let (dx, dy) =
      (fighter.aim_x - fighter.x - -60.0 * cos(t), fighter.aim_y - fighter.y - -60.0 * sin(t));
    let flip = body(&self.figure).scale_x;
    let tilt = self.figure.rotation;
    let weapon = arm_mut(&mut self.figure);
    weapon.y = -57.0;
    weapon.rotation = -tilt + round(180.0 - atan2(dx, dy) * 180.0 / PI);
    weapon.scale_x = flip;
  }

  /// The start of a step: the stun, the aim, the gravity, the fade of the drift, the ground under
  /// the feet and the kick.
  fn start(&mut self) {
    let fighter = &mut self.fighter;
    fighter.slope_y *= 0.95;
    if fighter.stunned != 0 {
      fighter.on_ledge = false;
      fighter.wall = 0;
      fighter.speed_x = (fighter.speed_x * 0.95).trunc();
      fighter.stunned -= 1;
    }
    self.aim();
    let (map, fighter) = (self.map, &mut self.fighter);
    let ground = map.hit(fighter.x, fighter.y);
    fighter.fall += 1.0;
    fighter.drift_y = (fighter.drift_y * 0.999).trunc();
    fighter.drift_x = (fighter.drift_x * 0.999).trunc();
    if fighter.wall != 0 {
      if fighter.on_ledge {
        fighter.slope_y = 0.0;
        if fighter.fall > 0.0 {
          fighter.fall = 0.0;
        }
      } else if fighter.fall > 2.0 {
        fighter.fall = 2.0;
      }
    } else if fighter.fall > 20.0 {
      fighter.fall = 20.0;
    }
    fighter.grounded = ground;
    if ground {
      while map.hit(fighter.x, fighter.y - 1.0) {
        fighter.y -= 1.0;
      }
      fighter.fall = 0.0;
    }
    if fighter.kick_x != 0.0 || fighter.kick_y != 0.0 {
      fighter.fall = 0.0;
      fighter.drift_x = fighter.kick_x.clamp(-40.0, 40.0);
      fighter.drift_y = fighter.kick_y.clamp(-40.0, 40.0);
      (fighter.kick_x, fighter.kick_y) = (0.0, 0.0);
    }
  }

  /// The keys of a fighter that is not stunned: the thrust, the run, the walls, the crouch and the
  /// jumps.
  fn steer(&mut self) {
    self.fighter.pose = Pose::Idle;
    if self.fighter.forced.is_none() {
      body_mut(&mut self.figure).scale_x =
        if self.fighter.aim_x < self.fighter.x { -1.0 } else { 1.0 };
    }
    let fighter = &mut self.fighter;
    if (fighter.grounded || fighter.wall != 0) && fighter.jump_wait != 0 {
      fighter.jump_wait -= 1;
    }
    if self.thrust && fighter.energy > 6.0 && !fighter.thrusted {
      fighter.energy -= 6.0;
      fighter.kick_y = -14.0;
      fighter.thrusted = true;
      fighter.brake = 10;
      let hand = self.hand();
      self.spawn(Effect::Thrust, hand, 0.0, 1.0);
    }
    let side = if self.held(Key::Left) { -1 } else { i32::from(self.held(Key::Right)) };
    if side != 0 {
      self.run(side);
    } else if self.held(Key::Down) && self.fighter.air_time < 5 {
      self.fighter.pose = Pose::Sit;
      arm_mut(&mut self.figure).y = -30.0;
    }
    let (up, fighter) = (self.held(Key::Up), &mut self.fighter);
    if fighter.wall == 0 && fighter.grounded && fighter.jump_wait == 0 && up {
      fighter.grounded = false;
      fighter.fall = (fighter.fall - 18.0 + (fighter.speed_x / 5.0).abs()).trunc();
      fighter.slope_y = 0.0;
      fighter.jump_wait = 5;
    }
    if fighter.on_ledge && fighter.jump_wait == 0 && up {
      fighter.fall -= 14.0;
      fighter.slope_y = 0.0;
      fighter.jump_wait = 3;
    }
  }

  /// A run to `side`: its speed, its slope, its pose, a jump off a wall behind, and a run up or a
  /// hold of a wall ahead.
  #[expect(clippy::float_cmp, reason = "a flip is exactly -1 or 1")]
  fn run(&mut self, side: i32) {
    let (up, flip) = (self.held(Key::Up), body(&self.figure).scale_x);
    let fighter = &mut self.fighter;
    let way = f64::from(side);
    let (most, gain) = if fighter.role == 1 { (12.0, 3.0) } else { (10.0, 2.0) };
    if fighter.speed_x * way < most {
      fighter.speed_x += gain * way;
    }
    if fighter.grounded && fighter.wall == 0 {
      fighter.slope_y = fighter.speed_x * sin(fighter.slope * PI / 180.0) * 1.2;
    }
    fighter.pose = if flip == way { Pose::Run } else { Pose::RunBack };
    if fighter.wall == -side {
      if !fighter.grounded && fighter.jump_wait == 0 && up {
        fighter.wall = 0;
        fighter.grounded = false;
        if fighter.fall > -10.0 {
          fighter.fall = -10.0;
        }
        fighter.slope_y = 0.0;
        fighter.speed_x = 15.0 * way;
        fighter.jump_wait = 1;
      }
    } else if fighter.wall == side {
      if fighter.fall >= -5.0 {
        fighter.forced = Some(Pose::Hang);
      } else if up {
        fighter.forced = Some(Pose::WallRun);
        fighter.jump_wait = 4;
        fighter.fall -= 1.0;
      }
    }
  }

  /// The fade of a run, the pose and the tilt in the air, the brake of the fall, and the angle of
  /// the ground under the feet.
  fn coast(&mut self) {
    let (map, flip) = (self.map, body(&self.figure).scale_x);
    let still = !self.held(Key::Right) && !self.held(Key::Left);
    let fighter = &mut self.fighter;
    if still {
      fighter.speed_x = (fighter.speed_x * 0.9).trunc();
    }
    if fighter.grounded {
      fighter.air_time = 0;
    } else {
      fighter.air_time += 1;
      if fighter.air_time > 5 {
        let speed = fighter.fall + fighter.drift_y;
        let amount = speed.clamp(-10.0, 10.0).abs().trunc();
        fighter.pose = if speed < -5.0 { Pose::Jump } else { Pose::Fall };
        if speed < 0.0 {
          fighter.tilt_target = 20.0 * (fighter.speed_x * amount / 100.0);
        } else if fighter.speed_x * flip > 0.0 {
          fighter.tilt_target = 20.0 * (-fighter.speed_x * amount / 100.0);
        }
      }
    }
    if fighter.brake != 0 {
      fighter.brake -= 1;
      if fighter.fall > 0.0 {
        let most = if fighter.brake > 5 { 0.0 } else { f64::from(5 - fighter.brake) };
        fighter.fall = fighter.fall.min(most);
      }
    }
    if fighter.grounded && fighter.wall == 0 {
      let mut angle = fighter.slope;
      for _ in 0..=60 {
        let (dx, dy) = (SIDE * cos(angle * PI / 180.0), SIDE * sin(angle * PI / 180.0));
        let left = map.hit(fighter.x - dx, fighter.y - dy + 1.0);
        let right = map.hit(fighter.x + dx, fighter.y + dy + 1.0);
        if left == right || !(-30.0..=30.0).contains(&angle) {
          break;
        }
        angle += if right { -1.0 } else { 1.0 };
      }
      fighter.slope = angle.clamp(-30.0, 30.0);
      fighter.tilt_target = fighter.slope + fighter.speed_x * 1.8;
    }
  }

  /// The move of the step, cut by the probes of the head, the feet and the sides of the body.
  fn collide(&mut self) -> Sides {
    let (map, fighter) = (self.map, &mut self.fighter);
    fighter.touching = false;
    fighter.step_x = fighter.speed_x + fighter.drift_x;
    fighter.step_y = fighter.fall + fighter.drift_y;
    if map.hit(fighter.x + fighter.step_x, fighter.y - 85.0 + fighter.step_y) {
      for value in [&mut fighter.fall, &mut fighter.drift_y, &mut fighter.step_y] {
        if *value < 0.0 {
          *value = 0.0;
        }
      }
    }
    if map.hit(fighter.x, fighter.y) {
      fighter.step_y = fighter.step_y.min(0.0);
      fighter.drift_y = fighter.drift_y.min(0.0);
    }
    let (left_x, right_x) = (fighter.x - (SIDE - 1.0), fighter.x + (SIDE - 1.0));
    let mut sides = Sides { row: 0, left: false, right: false };
    while sides.row < 4 {
      let dy = -83.0 + f64::from(sides.row) * 20.0;
      let top = fighter.y + dy;
      for j in 1..7 {
        let part = f64::from(j) / 6.0;
        let y = top + fighter.step_y * part;
        if map.hit(left_x + fighter.step_x * part, y) || map.hit(right_x + fighter.step_x * part, y)
        {
          fighter.step_x *= part - 1.0 / 6.0;
          (fighter.speed_x, fighter.drift_x) = (0.0, 0.0);
          break;
        }
      }
      sides.left = map.hit(fighter.x - REACH + fighter.step_x, fighter.y + dy + fighter.step_y);
      sides.right = map.hit(fighter.x + REACH + fighter.step_x, fighter.y + dy + fighter.step_y);
      if sides.left || sides.right {
        fighter.touching = true;
        break;
      }
      sides.row += 1;
    }
    if fighter.step_y > fighter.fall + fighter.slope_y {
      let (x, y, step) = (fighter.x, fighter.y, fighter.step_y);
      let ground =
        (1..5).map(|j| f64::from(j) / 4.0).filter(|part| map.hit(x, y + 1.0 + step * part));
      for part in ground.take(1) {
        fighter.step_y *= part - 1.0 / 4.0;
        (fighter.drift_y, fighter.slope_y, fighter.fall) = (0.0, 0.0, 0.0);
      }
    }
    sides
  }

  /// The hold of a wall or of a ledge that the top probes touch, and its end.
  fn cling(&mut self, sides: &Sides) {
    self.fighter.on_ledge = false;
    if self.fighter.stunned == 0 && !self.fighter.grounded && sides.row < 1 {
      for (side, touches, key) in [(-1, sides.left, Key::Left), (1, sides.right, Key::Right)] {
        if touches {
          self.hold(side, key);
        }
      }
    }
    let still = !self.held(Key::Right) && !self.held(Key::Left);
    let fighter = &mut self.fighter;
    if fighter.touching {
      if still && !fighter.on_ledge {
        fighter.wall = 0;
        fighter.forced = None;
      }
      if !fighter.grounded && fighter.wall != 0 {
        (fighter.slope, fighter.tilt, fighter.tilt_target) = (0.0, 0.0, 0.0);
      }
    } else {
      fighter.wall = 0;
      fighter.forced = fighter.forced.filter(|&pose| pose == Pose::Idle);
    }
    if fighter.stunned != 0 {
      fighter.forced = None;
    }
  }

  fn hold(&mut self, side: i32, key: Key) {
    let way = f64::from(side);
    if self.held(key) {
      self.fighter.wall = side;
      body_mut(&mut self.figure).scale_x = way;
      if matches!(self.fighter.forced, None | Some(Pose::Ledge)) {
        self.fighter.forced = Some(Pose::Hang);
      }
    }
    let fighter = &self.fighter;
    let above =
      (fighter.x + way * REACH * 1.3 + fighter.step_x, fighter.y - 105.0 + fighter.step_y);
    if !self.held(Key::Down) && !self.map.hit(above.0, above.1) {
      self.fighter.wall = side;
      body_mut(&mut self.figure).scale_x = way;
      self.fighter.on_ledge = true;
      if matches!(self.fighter.forced, None | Some(Pose::Hang)) {
        self.fighter.forced = Some(Pose::Ledge);
      }
    }
  }

  /// The move itself, the bounds of the world in x, the push out of a ceiling and of the walls,
  /// the halt, the tilt and the pose of the body.
  #[expect(clippy::float_cmp, reason = "a halt is a step that moves the feet by exactly nothing")]
  fn finish(&mut self) {
    let (map, side) = (self.map, SIDE + 1.0);
    let moving = self.held(Key::Left) || self.held(Key::Right);
    let fighter = &mut self.fighter;
    let (was_x, was_y) = (fighter.x, fighter.y);
    fighter.x += fighter.step_x;
    fighter.y = fighter.y + fighter.slope_y + fighter.step_y;
    fighter.x = fighter.x.max(20.0).min(f64::from(map.width) - 20.0);
    for _ in 0..20 {
      if !map.hit(fighter.x, fighter.y - 80.0) {
        break;
      }
      fighter.y += 1.0;
    }
    for _ in 0..20 {
      fighter.touching = false;
      for row in 0..4 {
        let dy = -80.0 + f64::from(row) * 20.0;
        let (left, right) =
          (map.hit(fighter.x - side, fighter.y + dy), map.hit(fighter.x + side, fighter.y + dy));
        if !left && !right {
          continue;
        }
        fighter.touching = true;
        let ceiling = row == 0 && left && right;
        if left && !ceiling {
          fighter.x += 1.0;
        }
        if right && !ceiling {
          fighter.x -= 1.0;
        }
      }
      if !fighter.touching {
        break;
      }
    }
    if fighter.forced == Some(Pose::Idle) {
      fighter.forced = None;
    }
    if was_x == fighter.x && was_y == fighter.y && !fighter.on_ledge && fighter.grounded && moving {
      fighter.tilt_target = 0.0;
      fighter.forced = Some(Pose::Idle);
    }
    fighter.tilt = approach(fighter.tilt, fighter.tilt_target, 0.2);
    self.figure.rotation = fighter.tilt;
    let want = fighter.forced.unwrap_or(fighter.pose).label();
    let spr = body_mut(&mut self.figure);
    if spr.label() != Some(want) {
      spr.play_from(At::Label(want));
    }
  }
}
