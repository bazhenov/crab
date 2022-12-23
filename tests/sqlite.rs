use crab::prelude::*;
use crab::storage::Storage;

#[tokio::test]
pub async fn example() -> Result<()> {
    let storage = Storage::new(":memory:").await?;
    assert_eq!("3.38.2", storage.version().await?);

    Ok(())
}
