pub mod storage;

pub mod prelude {
  
  pub type Result<T> = anyhow::Result<T>;
  pub use log::{trace, debug, warn, error};

  #[derive(Debug)]
  pub enum Error {
    
  }
}