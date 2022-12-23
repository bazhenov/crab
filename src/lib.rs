pub mod storage;

pub mod prelude {
  
  pub type Result<T> = anyhow::Result<T>;

  #[derive(Debug)]
  pub enum Error {
    
  }
}