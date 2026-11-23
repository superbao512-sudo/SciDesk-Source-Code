mod objects;
pub mod store;
pub mod server;
pub struct IdeasState(pub std::sync::Arc<server::Hub>);

#[cfg(test)]
mod tests;
