use std::time::Duration;

use futures::FutureExt;
use rust_commands::*;
use tokio::spawn;

struct Path;

struct Drivetrain;
struct Elevator;

impl Drivetrain {
  pub async fn manual_control(&mut self) {
    loop {
      // Do stuff in here
      println!("Drivetrain - Manual");
      tokio::time::sleep(Duration::from_millis(100)).await;
    }
  }

  pub async fn drive_path(&mut self, path: Path) {
    for _ in 0..10 {
      // Follow a path in here
      println!("Drivetrain - Path");
      tokio::time::sleep(Duration::from_millis(100)).await;
    }
    println!("Drivetrain - Path Done!");
  }
}

impl Elevator {
  pub async fn manual_control(&mut self) {
    loop {
      // Do stuff in here
      println!("Elevator - Manual");
      tokio::time::sleep(Duration::from_millis(100)).await;
    }
  }

  pub async fn go_to_height(&mut self, height: f64) {
    for _ in 0..5 {
      // Do control logic in here
      println!("Elevator - Height");
      tokio::time::sleep(Duration::from_millis(100)).await;
    }
    println!("Elevator - Height Done!");
  }
}

async fn dual_subsystem_command(systems: (&mut Drivetrain, &mut Elevator)) {
  for _ in 0..5 {
    // Do control logic in here
    println!("Dual - Tick");
    tokio::time::sleep(Duration::from_millis(100)).await;
  }
  println!("Dual - Done!");
}

#[derive(Systems)]
struct MySystems {
  drivetrain: Drivetrain,
  elevator: Elevator,
}

#[tokio::main]
pub async fn main() {
  let systems = MySystems {
    drivetrain: Drivetrain{},
    elevator: Elevator{},
  }.shared();

  perform!(systems.drivetrain, Priority(1), pinbox!(Drivetrain::manual_control));
  perform!(systems.elevator, Priority(1), pinbox!(Elevator::manual_control));

  tokio::time::sleep(Duration::from_millis(500)).await;

  perform!((systems.drivetrain, systems.elevator), Priority(10), pinbox!(dual_subsystem_command));

  tokio::time::sleep(Duration::from_millis(2000)).await;
}