use rsheet::RSheet;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rsheet = Arc::new(RSheet::new());
    let manager = rsheet::connect::TcpManager::new("127.0.0.1:8080".to_string());

    rsheet::start_server(rsheet, manager)?;

    Ok(())
}