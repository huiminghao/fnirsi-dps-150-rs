use anyhow::Result;
use dps150::*;
use tokio;
use tokio::time::Duration;
use tokio::{select, signal};

#[tokio::main(worker_threads = 2)]
async fn main() -> Result<()> {
    let mut controller = DPS150::new("/dev/ttyACM0")?;
    let mut timer = tokio::time::interval(Duration::from_secs(2));
    let mut output_enabled = false;
    controller.init_command().await;
    loop {
        select! {
            updated = controller.poll() => {
                println!("updated:{}", updated);
                if updated {
                    controller.print();
                }
                if !output_enabled {
                    controller.disable().await;
                    output_enabled = true;
                }
            }
            _ = timer.tick() => {
                // controller.print();
            }
            _ = signal::ctrl_c() => {
                break;
            }
        }
    }

    Ok(())
}
