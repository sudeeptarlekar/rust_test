use anyhow::Result;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration, Instant};

#[tokio::main]
async fn main() -> Result<()> {
    channel_select_example().await;
    Ok(())
}

async fn channel_select_example() {
    println!("=== Channel Select Example ===");

    let (tx1, mut rx1) = mpsc::channel::<String>(1);
    let (tx2, mut rx2) = mpsc::channel::<i32>(1);
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

    // Spawn background tasks
    let tx1_clone = tx1.clone();
    tokio::spawn(async move {
        sleep(Duration::from_secs(2)).await;
        let _ = tx1_clone.send("Hello from sender 1!".to_string()).await;
        println!("📨 Sender 1 sent message");
    });

    let tx2_clone = tx2.clone();
    tokio::spawn(async move {
        sleep(Duration::from_secs(4)).await;
        let _ = tx2_clone.send(42).await;
        println!("📨 Sender 2 sent message");
    });

    // This will trigger first
    tokio::spawn(async move {
        sleep(Duration::from_secs(10)).await;
        let _ = shutdown_tx.send(()).await;
        println!("🛑 Shutdown signal sent");
    });

    let start = Instant::now();

    tokio::select! {
        msg = rx1.recv() => {
            println!("📬 Received string message: {:?}", msg);
        }

        _ = shutdown_rx.recv() => {
            println!("🛑 Received shutdown signal - cancelling other operations");
        }

    }

    tokio::select! {
        num = rx2.recv() => {
            println!("📬 Received number message: {:?}", num);
        }

        _ = shutdown_rx.recv() => {
            println!("🛑 Received shutdown signal - cancelling other operations");
        }
    }

    println!("Select completed in: {:?}", start.elapsed());

    // Check if other channels still have pending messages (they shouldn't be processed)
    sleep(Duration::from_millis(100)).await;

    if let Ok(msg) = rx1.try_recv() {
        println!("⚠️  Unexpected: Found pending string message: {}", msg);
    } else {
        println!("✅ String channel was properly cancelled");
    }

    if let Ok(num) = rx2.try_recv() {
        println!("⚠️  Unexpected: Found pending number message: {}", num);
    } else {
        println!("✅ Number channel was properly cancelled");
    }

    println!();
}
